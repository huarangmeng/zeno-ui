use zeno_text::FontDescriptor;

use crate::{
    style::{Axis, Style},
    Node, NodeKind, SpacerNode, TextNode,
};

#[must_use]
pub fn text(content: impl Into<String>) -> Node {
    Node {
        kind: NodeKind::Text(TextNode {
            content: content.into(),
            font: FontDescriptor::default(),
            font_size: 16.0,
        }),
        style: Style::default(),
    }
}

#[must_use]
pub fn container(child: Node) -> Node {
    Node {
        kind: NodeKind::Container(Box::new(child)),
        style: Style::default(),
    }
}

#[must_use]
pub fn column(children: Vec<Node>) -> Node {
    Node {
        kind: NodeKind::Stack {
            axis: Axis::Vertical,
            children,
        },
        style: Style::default(),
    }
}

#[must_use]
pub fn row(children: Vec<Node>) -> Node {
    Node {
        kind: NodeKind::Stack {
            axis: Axis::Horizontal,
            children,
        },
        style: Style::default(),
    }
}

#[must_use]
pub fn spacer(width: f32, height: f32) -> Node {
    Node {
        kind: NodeKind::Spacer(SpacerNode { width, height }),
        style: Style::default(),
    }
}
