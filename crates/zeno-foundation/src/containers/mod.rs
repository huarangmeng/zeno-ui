use zeno_ui::{Axis, Node, NodeKind, SpacerNode};

use crate::id::next_node_id;

#[must_use]
pub fn container(child: Node) -> Node {
    Node::new(next_node_id(), NodeKind::Container(Box::new(child)))
}

#[must_use]
pub fn column(children: Vec<Node>) -> Node {
    Node::new(
        next_node_id(),
        NodeKind::Stack {
            axis: Axis::Vertical,
            children,
        },
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
    )
}

#[must_use]
pub fn spacer(width: f32, height: f32) -> Node {
    Node::new(next_node_id(), NodeKind::Spacer(SpacerNode { width, height }))
}
