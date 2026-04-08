use zeno_ui::{
    column, container, text, zeno_session_log, AppConfig, Color, DebugConfig, EdgeInsets,
    SceneSubmit, UiRuntime, WindowConfig,
};

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use zeno_ui::{DesktopShell, Shell};
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
use zeno_ui::{MinimalShell, Shell};

fn main() {
    let config = AppConfig {
        app_name: "minimal_app".to_string(),
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
    let root = container(
        column(vec![
            text("Zeno UI").font_size(28.0).foreground(Color::WHITE),
            text("Compose 风格声明式组件层").foreground(Color::WHITE),
            text("当前后端会自动优先选择 Impeller，否则回退到 Skia")
                .foreground(Color::rgba(230, 236, 255, 255)),
        ])
        .spacing(12.0),
    )
    .padding(EdgeInsets::horizontal_vertical(24.0, 20.0))
    .background(Color::rgba(39, 110, 241, 255))
    .corner_radius(24.0)
    .width(420.0);
    let mut runtime = UiRuntime::new(&zeno_ui::FallbackTextSystem);
    runtime.set_root(root);
    runtime.resize(config.window.size);
    let _first_frame = runtime.prepare_frame().expect("first frame").expect("scene");
    runtime.request_paint();
    let native_surface = DesktopShell.create_surface(&config.window);
    let (session, frame) = runtime
        .prepare_resolved_frame(native_surface.descriptor.platform, &config)
        .expect("resolved frame")
        .expect("second frame");
    let stats_after_paint = frame.compose_stats;
    let resource_count = frame.scene.resource_keys().len();
    let block_count = frame.scene.blocks.len();
    let (patch_upserts, patch_removes, scene_submit) = match frame.scene_submit {
        SceneSubmit::Full(scene) => (scene.blocks.len(), 0usize, SceneSubmit::Full(scene)),
        SceneSubmit::Patch { patch, current } => (
            patch.upserts.len(),
            patch.removes.len(),
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
            attempts = outcome.attempts.len(),
            compose_passes = stats_after_paint.compose_passes,
            layout_passes = stats_after_paint.layout_passes,
            cache_hits = stats_after_paint.cache_hits,
            resources = resource_count,
            blocks = block_count,
            patch_upserts,
            patch_removes,
            recomposed_after_invalidate = stats_after_paint.compose_passes > 1,
            "demo session summary"
        );
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = MinimalShell.create_surface(&config.window);
    }
}
