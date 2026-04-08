mod layout;
mod node;
mod render;
mod style;
mod widgets;

pub use node::{Node, NodeKind, SpacerNode, TextNode};
pub use render::{compose_scene, ComposeRenderer};
pub use style::{Axis, EdgeInsets, Style};
pub use widgets::{column, container, row, spacer, text};

#[cfg(test)]
mod tests {
    use super::{column, compose_scene, container, row, spacer, text};
    use zeno_core::{Color, Size};
    use zeno_graphics::DrawCommand;
    use zeno_text::FallbackTextSystem;

    #[test]
    fn builds_scene_from_column_tree() {
        let root = column(vec![
            text("Hello").font_size(20.0),
            spacer(0.0, 8.0),
            text("World"),
        ])
        .padding_all(12.0)
        .spacing(6.0)
        .background(Color::rgba(245, 247, 250, 255));

        let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);

        assert_eq!(scene.commands.len(), 3);
        assert!(matches!(scene.commands[0], DrawCommand::Fill { .. }));
        assert!(matches!(scene.commands[1], DrawCommand::Text { .. }));
        assert!(matches!(scene.commands[2], DrawCommand::Text { .. }));
    }

    #[test]
    fn builds_scene_from_nested_container_and_row() {
        let root = container(
            row(vec![text("A"), spacer(12.0, 0.0), text("B")])
                .spacing(8.0)
                .foreground(Color::WHITE),
        )
        .padding_all(16.0)
        .background(Color::rgba(39, 110, 241, 255))
        .corner_radius(18.0);

        let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);

        assert_eq!(scene.commands.len(), 3);
        assert!(matches!(
            scene.commands[0],
            DrawCommand::Fill {
                shape: zeno_graphics::Shape::RoundedRect { .. },
                ..
            }
        ));
    }
}
