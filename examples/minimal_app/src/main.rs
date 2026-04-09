use std::env;

use zeno_ui::{
    AppConfig, Backend, BackendPreference, Color, DebugConfig, EdgeInsets, Node, RendererConfig,
    SceneSubmit, UiRuntime, WindowConfig, column, container, text, zeno_session_log,
};

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use zeno_ui::{DesktopShell, Shell};
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
use zeno_ui::{MinimalShell, Shell};

fn main() {
    let config = AppConfig {
        app_name: "minimal_app".to_string(),
        renderer: renderer_config_from_env(),
        window: WindowConfig {
            title: "Zeno Minimal App".to_string(),
            ..WindowConfig::default()
        },
        debug: DebugConfig {
            frame_stats: true,
            ..DebugConfig::default()
        },
        ..AppConfig::default()
    };
    let mut runtime = UiRuntime::new(&zeno_ui::FallbackTextSystem);
    runtime.set_root(build_root(false));
    runtime.resize(config.window.size);
    let first_frame = runtime
        .prepare_frame()
        .expect("first frame")
        .expect("scene");
    runtime.set_root(build_root(true));
    let native_surface = DesktopShell.create_surface(&config.window);
    let (session, frame) = runtime
        .prepare_resolved_frame(native_surface.descriptor.platform, &config)
        .expect("resolved frame")
        .expect("second frame");
    let preview_submit = match &frame.scene_submit {
        SceneSubmit::Full(_) => "full",
        SceneSubmit::Patch { .. } => "patch",
    };
    let stats_after_paint = frame.compose_stats;
    let resource_count = frame.scene.resource_keys().len();
    let block_count = frame.scene.blocks.len();
    let (patch_upserts, patch_removes, scene_submit) = match frame.scene_submit {
        SceneSubmit::Full(scene) => (scene.blocks.len(), 0usize, SceneSubmit::Full(scene)),
        SceneSubmit::Patch { patch, current } => (
            patch.upserts.len()
                + patch.reorders.len()
                + patch.layer_upserts.len()
                + patch.layer_reorders.len(),
            patch.removes.len() + patch.layer_removes.len(),
            SceneSubmit::Patch { patch, current },
        ),
    };

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        let outcome = zeno_ui::ResolvedWindowRun {
            backend: session.backend.backend_kind,
            attempts: session.backend.attempts.clone(),
        };
        DesktopShell
            .run_pending_scene_window(session, scene_submit)
            .expect("desktop window should stay open until closed");
        zeno_session_log!(
            info,
            backend = ?outcome.backend,
            configured_preference = ?config.renderer.preference,
            attempts = outcome.attempts.len(),
            first_frame_commands = first_frame.scene.commands.len(),
            preview_submit,
            compose_passes = stats_after_paint.compose_passes,
            layout_passes = stats_after_paint.layout_passes,
            cache_hits = stats_after_paint.cache_hits,
            resources = resource_count,
            blocks = block_count,
            patch_upserts,
            patch_removes,
            recomposed_after_root_change = stats_after_paint.compose_passes > 1,
            "demo session summary"
        );
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = MinimalShell.create_surface(&config.window);
    }
}

fn build_root(accented: bool) -> Node {
    let title_color = if accented {
        Color::rgba(255, 244, 140, 255)
    } else {
        Color::WHITE
    };
    let container_color = if accented {
        Color::rgba(31, 92, 224, 255)
    } else {
        Color::rgba(39, 110, 241, 255)
    };
    container(
        column(vec![
            text("Zeno UI")
                .key("title")
                .font_size(28.0)
                .foreground(title_color),
            text("Compose 风格声明式组件层")
                .key("subtitle")
                .foreground(Color::WHITE),
            text("可通过 ZENO_DEMO_BACKEND=impeller|skia|prefer-skia|prefer-impeller 强制后端")
                .key("body")
                .foreground(Color::rgba(230, 236, 255, 255)),
        ])
        .spacing(12.0)
        .key("content"),
    )
    .key("root")
    .padding(EdgeInsets::horizontal_vertical(24.0, 20.0))
    .background(container_color)
    .corner_radius(24.0)
    .width(420.0)
}

fn renderer_config_from_env() -> RendererConfig {
    let Some(value) = env::var("ZENO_DEMO_BACKEND").ok() else {
        return RendererConfig::default();
    };
    let normalized = value.trim().to_ascii_lowercase();
    let preference = match normalized.as_str() {
        "auto" => BackendPreference::Auto,
        "prefer-impeller" => BackendPreference::PreferImpeller,
        "prefer-skia" => BackendPreference::PreferSkia,
        "impeller" => BackendPreference::Force(Backend::Impeller),
        "skia" => BackendPreference::Force(Backend::Skia),
        _ => BackendPreference::PreferImpeller,
    };
    RendererConfig {
        preference: preference.clone(),
        allow_fallback: !matches!(preference, BackendPreference::Force(_)),
    }
}
