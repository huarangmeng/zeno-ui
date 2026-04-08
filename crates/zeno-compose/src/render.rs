use zeno_core::{Point, Size};
use zeno_graphics::{Brush, DrawCommand, Scene, Shape};
use zeno_text::TextSystem;

use crate::{
    layout::{measure_node, MeasuredKind, MeasuredNode},
    Node, NodeKind,
};

pub struct ComposeRenderer<'a> {
    text_system: &'a dyn TextSystem,
}

impl<'a> ComposeRenderer<'a> {
    #[must_use]
    pub fn new(text_system: &'a dyn TextSystem) -> Self {
        Self { text_system }
    }

    #[must_use]
    pub fn compose(&self, root: &Node, viewport: Size) -> Scene {
        let measured = measure_node(root, Point::new(0.0, 0.0), viewport, self.text_system);
        let mut scene = Scene::new(viewport);
        emit_node(root, &measured, &mut scene);
        scene
    }
}

#[must_use]
pub fn compose_scene(root: &Node, viewport: Size, text_system: &dyn TextSystem) -> Scene {
    ComposeRenderer::new(text_system).compose(root, viewport)
}

fn emit_node(node: &Node, measured: &MeasuredNode, scene: &mut Scene) {
    if let Some(background) = node.style.background {
        let shape = if node.style.corner_radius > 0.0 {
            Shape::RoundedRect {
                rect: measured.frame,
                radius: node.style.corner_radius,
            }
        } else {
            Shape::Rect(measured.frame)
        };
        scene.push(DrawCommand::Fill {
            shape,
            brush: Brush::Solid(background),
        });
    }

    match (&node.kind, &measured.kind) {
        (NodeKind::Text(_), MeasuredKind::Text(layout)) => {
            let position = Point::new(
                measured.frame.origin.x + node.style.padding.left,
                measured.frame.origin.y + node.style.padding.top + layout.paragraph.font_size,
            );
            scene.push(DrawCommand::Text {
                position,
                layout: layout.clone(),
                color: node.style.foreground,
            });
        }
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            emit_node(child, measured_child, scene);
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            for (child, measured_child) in children.iter().zip(measured_children.iter()) {
                emit_node(child, measured_child, scene);
            }
        }
        (NodeKind::Spacer(_), MeasuredKind::Spacer) => {}
        _ => {}
    }
}
