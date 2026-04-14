use zeno_ui::{Node, NodeKind};

use crate::id::next_node_id;

#[must_use]
pub fn container(child: impl Into<Node>) -> Node {
    Node::new(next_node_id(), NodeKind::Container(Box::new(child.into())))
}
