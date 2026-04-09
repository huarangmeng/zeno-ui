use std::{env, fs, path::Path, process};

use zeno_core::{Color, Size};
use zeno_foundation::{column, container, text};
use zeno_runtime::UiRuntime;
use zeno_scene::SceneSubmit;
use zeno_text::{SystemTextSystem, TextSystem};
use zeno_ui::{EdgeInsets, dump_layout, dump_scene, Node};

fn main() {
    let iterations = env::var("ZENO_TEXT_PROBE_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(6);
    let output_format = env::var("ZENO_TEXT_PROBE_FORMAT").unwrap_or_else(|_| "plain".to_string());
    let output_path = env::var("ZENO_TEXT_PROBE_OUTPUT").ok();
    let min_patch_ratio = env::var("ZENO_TEXT_PROBE_MIN_PATCH_RATIO")
        .ok()
        .and_then(|value| value.parse::<f32>().ok());
    let min_cache_hit_rate = env::var("ZENO_TEXT_PROBE_MIN_CACHE_HIT_RATE")
        .ok()
        .and_then(|value| value.parse::<f32>().ok());
    let dump_mode = env::var("ZENO_TEXT_PROBE_DUMP").unwrap_or_else(|_| "none".to_string());
    let variant = env::var("ZENO_TEXT_PROBE_VARIANT").unwrap_or_else(|_| "edit".to_string());
    let viewport = Size::new(640.0, 480.0);
    let text_system = SystemTextSystem;
    text_system.reset_caches();

    let mut runtime = UiRuntime::new(&text_system);
    runtime.resize(viewport);

    let mut last_root = build_root(&variant, 0);
    let mut full_frames = 0usize;
    let mut patch_frames = 0usize;
    let mut total_commands = 0usize;
    let mut total_blocks = 0usize;

    for index in 0..iterations {
        last_root = build_root(&variant, index);
        runtime.set_root(last_root.clone());
        if let Some(frame) = runtime.prepare_frame().expect("text probe frame") {
            match frame.scene_submit {
                SceneSubmit::Full(_) => full_frames += 1,
                SceneSubmit::Patch { .. } => patch_frames += 1,
            }
            total_commands += frame.scene.commands.len();
            total_blocks += frame.scene.blocks.len();
            if matches!(dump_mode.as_str(), "scene" | "all") && index + 1 == iterations {
                println!("--- scene dump ---\n{}", dump_scene(&frame.scene));
            }
        }
    }

    if matches!(dump_mode.as_str(), "layout" | "all") {
        println!(
            "--- layout dump ---\n{}",
            dump_layout(&last_root, viewport, &text_system)
        );
    }

    let cache = text_system.cache_stats().expect("fallback cache stats");
    let metrics = ProbeMetrics {
        variant,
        iterations,
        full_frames,
        patch_frames,
        avg_commands: total_commands as f32 / iterations.max(1) as f32,
        avg_blocks: total_blocks as f32 / iterations.max(1) as f32,
        cache_entries: cache.entries,
        cache_hits: cache.hits,
        cache_misses: cache.misses,
    };
    let output = metrics.format(&output_format);
    if let Some(output_path) = output_path {
        write_output(&output_path, &output);
    }
    println!("{output}");
    if let Err(message) = metrics.assert_thresholds(min_patch_ratio, min_cache_hit_rate) {
        eprintln!("{message}");
        process::exit(1);
    }
}

fn build_root(variant: &str, iteration: usize) -> Node {
    let body = match variant {
        "font-switch" => {
            let size = if iteration % 2 == 0 { 16.0 } else { 26.0 };
            text("Font switch probe with repeated layout and cache hits.")
                .key("body")
                .font_size(size)
        }
        "transform" => {
            let rotation = if iteration % 2 == 0 { 0.0 } else { 6.0 };
            text("Transform probe keeps text content stable while modifiers change.")
                .key("body")
                .font_size(18.0)
                .rotate_degrees(rotation)
        }
        _ => {
            let content = if iteration % 2 == 0 {
                "Edit probe keeps one paragraph stable and flips another paragraph to exercise paragraph caching."
            } else {
                "Edit probe keeps one paragraph stable and flips a shorter body to force incremental scene/text updates."
            };
            text(content).key("body").font_size(18.0)
        }
    };

    container(
        column(vec![
            text("Text Probe")
                .key("title")
                .font_size(28.0)
                .foreground(Color::WHITE),
            text("Exercises TextSystem / TextShaper / TextCache and scene dumps.")
                .key("subtitle")
                .foreground(Color::rgba(235, 240, 255, 255)),
            body.foreground(Color::rgba(255, 244, 214, 255)),
        ])
        .spacing(12.0)
        .key("content"),
    )
    .key("root")
    .padding(EdgeInsets::horizontal_vertical(24.0, 20.0))
    .background(Color::rgba(43, 89, 222, 255))
    .corner_radius(24.0)
    .width(560.0)
}

struct ProbeMetrics {
    variant: String,
    iterations: usize,
    full_frames: usize,
    patch_frames: usize,
    avg_commands: f32,
    avg_blocks: f32,
    cache_entries: usize,
    cache_hits: usize,
    cache_misses: usize,
}

impl ProbeMetrics {
    fn patch_ratio(&self) -> f32 {
        self.patch_frames as f32 / self.iterations.max(1) as f32
    }

    fn cache_hit_rate(&self) -> f32 {
        let accesses = self.cache_hits + self.cache_misses;
        self.cache_hits as f32 / accesses.max(1) as f32
    }

    fn format(&self, output_format: &str) -> String {
        match output_format {
            "json" => format!(
                concat!(
                    "{{",
                    "\"variant\":\"{}\",",
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
                self.variant,
                self.iterations,
                self.full_frames,
                self.patch_frames,
                self.patch_ratio(),
                self.avg_commands,
                self.avg_blocks,
                self.cache_entries,
                self.cache_hits,
                self.cache_misses,
                self.cache_hit_rate(),
            ),
            "csv" => format!(
                concat!(
                    "variant,iterations,full_frames,patch_frames,patch_ratio,avg_commands,avg_blocks,cache_entries,cache_hits,cache_misses,cache_hit_rate\n",
                    "{},{},{},{},{:.4},{:.2},{:.2},{},{},{},{:.4}"
                ),
                self.variant,
                self.iterations,
                self.full_frames,
                self.patch_frames,
                self.patch_ratio(),
                self.avg_commands,
                self.avg_blocks,
                self.cache_entries,
                self.cache_hits,
                self.cache_misses,
                self.cache_hit_rate(),
            ),
            _ => format!(
                "text_probe variant={} iterations={} full_frames={} patch_frames={} patch_ratio={:.4} avg_commands={:.2} avg_blocks={:.2} cache_entries={} cache_hits={} cache_misses={} cache_hit_rate={:.4}",
                self.variant,
                self.iterations,
                self.full_frames,
                self.patch_frames,
                self.patch_ratio(),
                self.avg_commands,
                self.avg_blocks,
                self.cache_entries,
                self.cache_hits,
                self.cache_misses,
                self.cache_hit_rate(),
            ),
        }
    }

    fn assert_thresholds(
        &self,
        min_patch_ratio: Option<f32>,
        min_cache_hit_rate: Option<f32>,
    ) -> Result<(), String> {
        if let Some(threshold) = min_patch_ratio
            && self.patch_ratio() < threshold
        {
            return Err(format!(
                "text_probe patch_ratio {:.4} below threshold {:.4}",
                self.patch_ratio(),
                threshold
            ));
        }
        if let Some(threshold) = min_cache_hit_rate
            && self.cache_hit_rate() < threshold
        {
            return Err(format!(
                "text_probe cache_hit_rate {:.4} below threshold {:.4}",
                self.cache_hit_rate(),
                threshold
            ));
        }
        Ok(())
    }
}

fn write_output(path: &str, output: &str) {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).expect("create text probe output directory");
        }
    }
    fs::write(path, output).expect("write text probe output");
}
