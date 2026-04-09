use zeno_ui::{Node, NodeKind};

use crate::id::next_node_id;

#[must_use]
pub fn r#box(children: Vec<Node>) -> Node {
    Node::new(next_node_id(), NodeKind::Box { children })
}
