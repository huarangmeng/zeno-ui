use zeno_ui::{
    column, compose_scene, container, text, AppConfig, BackendResolver, Color, DrawCommand,
    EdgeInsets, WindowConfig,
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
        ..AppConfig::default()
    };
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    let native_surface = {
        let shell = DesktopShell;
        shell.create_surface(&config.window)
    };

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let native_surface = {
        let shell = MinimalShell;
        shell.create_surface(&config.window)
    };
    let resolver = BackendResolver::new();
    let resolved = resolver
        .resolve(native_surface.descriptor.platform, &config.renderer)
        .expect("renderer should resolve");
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
    let mut scene = compose_scene(&root, config.window.size, &zeno_ui::FallbackTextSystem);
    scene.push(DrawCommand::Clear(Color::WHITE));
    scene.commands.rotate_right(1);

    println!(
        "platform={} backend={} commands={} surface={}",
        native_surface.descriptor.platform,
        resolved.backend_kind,
        scene.commands.len(),
        native_surface.surface.id
    );

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    DesktopShell
        .run_backend_scene_window(&config.window, resolved.backend_kind, scene)
        .expect("desktop window should stay open until closed");
}
