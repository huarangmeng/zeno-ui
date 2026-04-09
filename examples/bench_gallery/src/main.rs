use std::{env, fs, path::Path, process};

use zeno_core::{Color, Size};
use zeno_foundation::{column, container, row, spacer, text};
use zeno_runtime::UiRuntime;
use zeno_scene::SceneSubmit;
use zeno_text::{SystemTextSystem, TextSystem};
use zeno_ui::{BlendMode, EdgeInsets, Node};

fn main() {
    let iterations = env::var("ZENO_BENCH_GALLERY_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(8);
    let output_format =
        env::var("ZENO_BENCH_GALLERY_FORMAT").unwrap_or_else(|_| "plain".to_string());
    let output_path = env::var("ZENO_BENCH_GALLERY_OUTPUT").ok();
    let min_patch_ratio = env::var("ZENO_BENCH_GALLERY_MIN_PATCH_RATIO")
        .ok()
        .and_then(|value| value.parse::<f32>().ok());
    let min_cache_hit_rate = env::var("ZENO_BENCH_GALLERY_MIN_CACHE_HIT_RATE")
        .ok()
        .and_then(|value| value.parse::<f32>().ok());
    let viewport = Size::new(960.0, 720.0);
    let text_system = SystemTextSystem;
    let scenarios = [
        Scenario::DeepTree,
        Scenario::LongText,
        Scenario::EffectStack,
        Scenario::RapidPatch,
    ];
    let mut reports = Vec::with_capacity(scenarios.len());

    for scenario in scenarios {
        text_system.reset_caches();
        let mut runtime = UiRuntime::new(&text_system);
        runtime.resize(viewport);
        let mut full_frames = 0usize;
        let mut patch_frames = 0usize;
        let mut total_commands = 0usize;
        let mut total_blocks = 0usize;

        for iteration in 0..iterations {
            runtime.set_root(scenario.build(iteration));
            if let Some(frame) = runtime.prepare_frame().expect("bench gallery frame") {
                match frame.scene_submit {
                    SceneSubmit::Full(_) => full_frames += 1,
                    SceneSubmit::Patch { .. } => patch_frames += 1,
                }
                total_commands += frame.scene.command_count();
                total_blocks += frame.scene.blocks.len();
            }
        }

        let cache = text_system.cache_stats().expect("bench gallery text cache");
        reports.push(ScenarioReport {
            scenario: scenario.as_str(),
            iterations,
            full_frames,
            patch_frames,
            avg_commands: total_commands as f32 / iterations.max(1) as f32,
            avg_blocks: total_blocks as f32 / iterations.max(1) as f32,
            cache_entries: cache.entries,
            cache_hits: cache.hits,
            cache_misses: cache.misses,
        });
    }

    let summary = BenchGalleryReport { reports };
    let output = summary.format(&output_format);
    if let Some(output_path) = output_path {
        write_output(&output_path, &output);
    }
    println!("{output}");
    if let Err(message) = summary.assert_thresholds(min_patch_ratio, min_cache_hit_rate) {
        eprintln!("{message}");
        process::exit(1);
    }
}

#[derive(Clone, Copy)]
enum Scenario {
    DeepTree,
    LongText,
    EffectStack,
    RapidPatch,
}

impl Scenario {
    fn as_str(self) -> &'static str {
        match self {
            Self::DeepTree => "deep-tree",
            Self::LongText => "long-text",
            Self::EffectStack => "effect-stack",
            Self::RapidPatch => "rapid-patch",
        }
    }

    fn build(self, iteration: usize) -> Node {
        match self {
            Self::DeepTree => deep_tree_root(iteration),
            Self::LongText => long_text_root(iteration),
            Self::EffectStack => effect_stack_root(iteration),
            Self::RapidPatch => rapid_patch_root(iteration),
        }
    }
}

struct ScenarioReport {
    scenario: &'static str,
    iterations: usize,
    full_frames: usize,
    patch_frames: usize,
    avg_commands: f32,
    avg_blocks: f32,
    cache_entries: usize,
    cache_hits: usize,
    cache_misses: usize,
}

impl ScenarioReport {
    fn patch_ratio(&self) -> f32 {
        self.patch_frames as f32 / self.iterations.max(1) as f32
    }

    fn cache_hit_rate(&self) -> f32 {
        let accesses = self.cache_hits + self.cache_misses;
        self.cache_hits as f32 / accesses.max(1) as f32
    }
}

struct BenchGalleryReport {
    reports: Vec<ScenarioReport>,
}

impl BenchGalleryReport {
    fn format(&self, output_format: &str) -> String {
        match output_format {
            "json" => {
                let items = self
                    .reports
                    .iter()
                    .map(|report| {
                        format!(
                            concat!(
                                "{{",
                                "\"scenario\":\"{}\",",
                                "\"iterations\":{},",
                                "\"full_frames\":{},",
                                "\"patch_frames\":{},",
                                "\"patch_ratio\":{:.4},",
                                "\"avg_commands\":{:.2},",
                                "\"avg_blocks\":{:.2},",
                                "\"cache_entries\":{},",
                                "\"cache_hits\":{},",
                                "\"cache_misses\":{},",
                                "\"cache_hit_rate\":{:.4}",
                                "}}"
                            ),
                            report.scenario,
                            report.iterations,
                            report.full_frames,
                            report.patch_frames,
                            report.patch_ratio(),
                            report.avg_commands,
                            report.avg_blocks,
                            report.cache_entries,
                            report.cache_hits,
                            report.cache_misses,
                            report.cache_hit_rate(),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{\"reports\":[{items}]}}")
            }
            "csv" => {
                let mut csv = String::from(
                    "scenario,iterations,full_frames,patch_frames,patch_ratio,avg_commands,avg_blocks,cache_entries,cache_hits,cache_misses,cache_hit_rate\n",
                );
                for report in &self.reports {
                    csv.push_str(&format!(
                        "{},{},{},{},{:.4},{:.2},{:.2},{},{},{},{:.4}\n",
                        report.scenario,
                        report.iterations,
                        report.full_frames,
                        report.patch_frames,
                        report.patch_ratio(),
                        report.avg_commands,
                        report.avg_blocks,
                        report.cache_entries,
                        report.cache_hits,
                        report.cache_misses,
                        report.cache_hit_rate(),
                    ));
                }
                csv.trim_end().to_string()
            }
            _ => self
                .reports
                .iter()
                .map(|report| {
                    format!(
                        "bench_gallery scenario={} iterations={} full_frames={} patch_frames={} patch_ratio={:.4} avg_commands={:.2} avg_blocks={:.2} cache_entries={} cache_hits={} cache_misses={} cache_hit_rate={:.4}",
                        report.scenario,
                        report.iterations,
                        report.full_frames,
                        report.patch_frames,
                        report.patch_ratio(),
                        report.avg_commands,
                        report.avg_blocks,
                        report.cache_entries,
                        report.cache_hits,
                        report.cache_misses,
                        report.cache_hit_rate(),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    fn assert_thresholds(
        &self,
        min_patch_ratio: Option<f32>,
        min_cache_hit_rate: Option<f32>,
    ) -> Result<(), String> {
        for report in &self.reports {
            if let Some(threshold) = min_patch_ratio
                && report.patch_ratio() < threshold
            {
                return Err(format!(
                    "bench_gallery {} patch_ratio {:.4} below threshold {:.4}",
                    report.scenario,
                    report.patch_ratio(),
                    threshold
                ));
            }
            if let Some(threshold) = min_cache_hit_rate
                && report.cache_hit_rate() < threshold
            {
                return Err(format!(
                    "bench_gallery {} cache_hit_rate {:.4} below threshold {:.4}",
                    report.scenario,
                    report.cache_hit_rate(),
                    threshold
                ));
            }
        }
        Ok(())
    }
}

fn deep_tree_root(iteration: usize) -> Node {
    let mut branch = text("leaf 0")
        .key("leaf-0")
        .font_size(16.0 + (iteration % 3) as f32);
    for depth in 1..=12 {
        branch = container(branch)
            .key(format!("deep-{depth}"))
            .padding(EdgeInsets::all(6.0 + depth as f32))
            .background(Color::rgba(20 + depth as u8 * 5, 40, 120, 255))
            .corner_radius(8.0)
            .width(220.0 + depth as f32 * 18.0);
    }
    container(branch)
        .key("deep-root")
        .padding(EdgeInsets::all(24.0))
        .background(Color::rgba(245, 247, 252, 255))
}

fn long_text_root(iteration: usize) -> Node {
    let emphasis = if iteration.is_multiple_of(2) {
        "system shaping"
    } else {
        "paragraph cache"
    };
    container(
        column(vec![
            text("Long text bench").key("title").font_size(30.0),
            text(format!(
                "This scenario stresses {emphasis} across repeated paragraphs while keeping the container tree stable. The runtime should prefer patch updates and the text cache should accumulate hits over repeated frames."
            ))
            .key("body-0")
            .font_size(18.0),
            text(
                "第二段文本用于覆盖中英文混排、换行和宽度约束，让同一个 TextSystem 在连续帧里既有缓存命中，也有局部内容更新。",
            )
            .key("body-1")
            .font_size(20.0),
        ])
        .spacing(18.0)
        .key("long-text-column"),
    )
    .key("long-text-root")
    .padding(EdgeInsets::all(28.0))
    .background(Color::rgba(235, 241, 255, 255))
    .width(720.0)
}

fn effect_stack_root(iteration: usize) -> Node {
    let cards = (0..4)
        .map(|index| {
            let rotation = if (iteration + index).is_multiple_of(2) {
                0.0
            } else {
                3.0 + index as f32
            };
            container(
                text(format!("effect card {}", index + 1))
                    .key(format!("effect-text-{index}"))
                    .font_size(18.0)
                    .foreground(Color::WHITE),
            )
            .key(format!("effect-card-{index}"))
            .padding(EdgeInsets::all(18.0))
            .background(Color::rgba(59, 92 + index as u8 * 20, 211, 255))
            .corner_radius(18.0)
            .blur(6.0 + index as f32)
            .drop_shadow(4.0, 6.0, 8.0 + index as f32, Color::rgba(0, 0, 0, 96))
            .blend_mode(if index.is_multiple_of(2) {
                BlendMode::Normal
            } else {
                BlendMode::Screen
            })
            .rotate_degrees(rotation)
            .width(180.0)
        })
        .collect::<Vec<_>>();
    container(row(cards).spacing(18.0).key("effect-row"))
        .key("effect-root")
        .padding(EdgeInsets::all(24.0))
        .background(Color::rgba(18, 22, 38, 255))
}

fn rapid_patch_root(iteration: usize) -> Node {
    let labels = if iteration.is_multiple_of(2) {
        ["A", "B", "C", "D"]
    } else {
        ["B", "A", "D", "C"]
    };
    container(
        row(vec![
            text(labels[0]).key("rapid-a").font_size(26.0),
            spacer(12.0, 0.0),
            text(labels[1]).key("rapid-b").font_size(26.0),
            spacer(12.0, 0.0),
            text(labels[2]).key("rapid-c").font_size(26.0),
            spacer(12.0, 0.0),
            text(labels[3]).key("rapid-d").font_size(26.0),
        ])
        .spacing(8.0)
        .key("rapid-row"),
    )
    .key("rapid-root")
    .padding(EdgeInsets::all(20.0))
    .background(Color::rgba(255, 248, 229, 255))
}

fn write_output(path: &str, output: &str) {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).expect("create bench gallery output directory");
        }
    }
    fs::write(path, output).expect("write bench gallery output");
}
