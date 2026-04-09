use std::env;

use zeno_ui::{
    Color, EdgeInsets, FallbackTextSystem, SceneSubmit, Size, TextSystem, UiRuntime, column,
    container, dump_layout, dump_scene, text,
};

fn main() {
    let iterations = env::var("ZENO_TEXT_PROBE_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(6);
    let dump_mode = env::var("ZENO_TEXT_PROBE_DUMP").unwrap_or_else(|_| "none".to_string());
    let variant = env::var("ZENO_TEXT_PROBE_VARIANT").unwrap_or_else(|_| "edit".to_string());
    let viewport = Size::new(640.0, 480.0);
    let text_system = FallbackTextSystem;
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
    println!(
        "text_probe iterations={} variant={} full_frames={} patch_frames={} avg_commands={:.2} avg_blocks={:.2} cache_entries={} cache_hits={} cache_misses={}",
        iterations,
        variant,
        full_frames,
        patch_frames,
        total_commands as f32 / iterations.max(1) as f32,
        total_blocks as f32 / iterations.max(1) as f32,
        cache.entries,
        cache.hits,
        cache.misses
    );
}

fn build_root(variant: &str, iteration: usize) -> zeno_ui::Node {
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
