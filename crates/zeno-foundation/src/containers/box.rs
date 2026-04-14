use zeno_ui::{Node, NodeKind};

use crate::id::next_node_id;

#[must_use]
pub fn r#box<I, T>(children: I) -> Node
where
    I: IntoIterator<Item = T>,
    T: Into<Node>,
{
    Node::new(
        next_node_id(),
        NodeKind::Box {
            children: children.into_iter().map(Into::into).collect(),
        },
    )
}
