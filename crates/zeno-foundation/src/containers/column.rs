use zeno_ui::{Axis, Node, NodeKind};

use crate::id::next_node_id;

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
