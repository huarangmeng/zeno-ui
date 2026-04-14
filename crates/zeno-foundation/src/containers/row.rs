use zeno_ui::{Axis, Node, NodeKind};

use crate::id::next_node_id;

#[must_use]
pub fn row<I, T>(children: I) -> Node
where
    I: IntoIterator<Item = T>,
    T: Into<Node>,
{
    Node::new(
        next_node_id(),
        NodeKind::Stack {
            axis: Axis::Horizontal,
            children: children.into_iter().map(Into::into).collect(),
        },
    )
}
