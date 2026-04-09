use zeno_ui::{Node, NodeKind, SpacerNode};

use crate::id::next_node_id;

#[must_use]
pub fn spacer(width: f32, height: f32) -> Node {
    Node::new(next_node_id(), NodeKind::Spacer(SpacerNode { width, height }))
}
