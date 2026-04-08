use std::sync::atomic::{AtomicU64, Ordering};

use zeno_text::FontDescriptor;

use crate::{
    node::NodeId,
    style::{Axis, Style},
    Node, NodeKind, SpacerNode, TextNode,
};

static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);

fn next_node_id() -> NodeId {
    NodeId(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed))
}

#[must_use]
pub fn text(content: impl Into<String>) -> Node {
    Node::new(
        next_node_id(),
        NodeKind::Text(TextNode {
            content: content.into(),
            font: FontDescriptor::default(),
            font_size: 16.0,
        }),
        Style::default(),
    )
}

#[must_use]
pub fn container(child: Node) -> Node {
    Node::new(
        next_node_id(),
        NodeKind::Container(Box::new(child)),
        Style::default(),
    )
}

#[must_use]
pub fn column(children: Vec<Node>) -> Node {
    Node::new(
        next_node_id(),
        NodeKind::Stack {
            axis: Axis::Vertical,
            children,
        },
        Style::default(),
    )
}

#[must_use]
pub fn row(children: Vec<Node>) -> Node {
    Node::new(
        next_node_id(),
        NodeKind::Stack {
            axis: Axis::Horizontal,
            children,
        },
        Style::default(),
    )
}

#[must_use]
pub fn spacer(width: f32, height: f32) -> Node {
    Node::new(
        next_node_id(),
        NodeKind::Spacer(SpacerNode { width, height }),
        Style::default(),
    )
}
