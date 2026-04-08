use zeno_shell::{MinimalShell, Shell};
use zeno_text::{FallbackTextSystem, TextParagraph, TextSystem};
use zeno_ui::{
    AppConfig, BackendResolver, Brush, Color, DrawCommand, Point, Scene, Shape, WindowConfig,
};

fn main() {
    let config = AppConfig {
        app_name: "minimal_app".to_string(),
        window: WindowConfig {
            title: "Zeno Minimal App".to_string(),
            ..WindowConfig::default()
        },
        ..AppConfig::default()
    };
    let shell = MinimalShell;
    let native_surface = shell.create_surface(&config.window);
    let resolver = BackendResolver::new();
    let resolved = resolver
        .resolve(native_surface.descriptor.platform, &config.renderer)
        .expect("renderer should resolve");
    let text_system = FallbackTextSystem;
    let layout = text_system.layout(TextParagraph::new("Zeno UI", 300.0));

    let mut scene = Scene::new(config.window.size);
    scene.push(DrawCommand::Clear(Color::WHITE));
    scene.push(DrawCommand::Fill {
        shape: Shape::RoundedRect {
            rect: zeno_ui::Rect::new(32.0, 32.0, 280.0, 120.0),
            radius: 24.0,
        },
        brush: Brush::Solid(Color::rgba(39, 110, 241, 255)),
    });
    scene.push(DrawCommand::Text {
        position: Point::new(56.0, 96.0),
        layout,
        color: Color::BLACK,
    });

    let report = resolved
        .renderer
        .render(&native_surface.surface, &scene)
        .expect("render should succeed");

    println!(
        "platform={} backend={} commands={} surface={}",
        native_surface.descriptor.platform,
        report.backend,
        report.command_count,
        report.surface_id
    );
}
