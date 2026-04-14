use zeno_ui::{Alignment, Axis, InteractionRole, Modifier, Node, NodeKind};

use crate::id::next_node_id;

#[must_use]
pub fn scroll(axis: Axis, offset: f32, child: impl Into<Node>) -> Node {
    let child = child.into();
    let (x, y) = match axis {
        Axis::Horizontal => (-offset.max(0.0), 0.0),
        Axis::Vertical => (0.0, -offset.max(0.0)),
    };

    let node = Node::new(
        next_node_id(),
        NodeKind::Container(Box::new(child.translate(x, y))),
    )
    .modifier(Modifier::InteractionRole(InteractionRole::Scroll))
    .content_alignment(Alignment::TOP_START);

    node.clip()
}
