use zeno_ui::{Node, NodeKind, TextNode};

use crate::id::next_node_id;

#[must_use]
pub fn text(content: impl Into<String>) -> Node {
    Node::new(
        next_node_id(),
        NodeKind::Text(TextNode {
            content: content.into(),
        }),
    )
}
