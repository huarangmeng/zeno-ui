mod arena;
mod geometry;
mod relayout;
mod work_queue;

use zeno_core::{Rect, Size};
use crate::Node;

pub(crate) use arena::{
    measure_layout, measure_node, LayoutArena, LayoutSlot, MeasuredKind, MeasuredNode,
};
pub(crate) use geometry::{
    aligned_offset, aligned_offset_for_cross_axis, arranged_gap_and_offset, main_axis_extent,
    remaining_available_for_axis, stack_content_size, stack_cross_extent,
};
pub(crate) use relayout::relayout_layout;

#[derive(Debug, Clone)]
pub(crate) struct NodeLayoutData {
    pub(crate) frame: Rect,
}

#[allow(dead_code)]
pub(crate) fn content_available(node: &Node, available: Size) -> Size {
    let style = node.resolved_style();
    finalize_content_available(style.padding.horizontal(), style.padding.vertical(), available)
}

fn finalize_content_available(padding_h: f32, padding_v: f32, available: Size) -> Size {
    geometry::content_available(padding_h, padding_v, available)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Axis, EdgeInsets, Node, NodeId, NodeKind, SpacerNode};
    use zeno_core::{Point, Size};

    fn next_node_id() -> NodeId {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);
        NodeId(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed))
    }

    fn spacer(width: f32, height: f32) -> Node {
        Node::new(next_node_id(), NodeKind::Spacer(SpacerNode { width, height }))
    }

    fn container(child: Node) -> Node {
        Node::new(next_node_id(), NodeKind::Container(Box::new(child)))
    }

    fn row(children: Vec<Node>) -> Node {
        Node::new(
            next_node_id(),
            NodeKind::Stack {
                axis: Axis::Horizontal,
                children,
            },
        )
    }

    #[test]
    fn content_available_subtracts_padding_once() {
        let node = container(spacer(10.0, 10.0)).padding(EdgeInsets {
            left: 4.0,
            top: 6.0,
            right: 8.0,
            bottom: 10.0,
        });

        let available = content_available(&node, Size::new(80.0, 50.0));

        assert_eq!(available, Size::new(68.0, 34.0));
    }

    #[test]
    fn remaining_available_for_axis_clamps_on_main_axis() {
        assert_eq!(
            remaining_available_for_axis(Size::new(40.0, 20.0), 55.0, Axis::Horizontal),
            Size::new(0.0, 20.0)
        );
        assert_eq!(
            remaining_available_for_axis(Size::new(40.0, 20.0), 55.0, Axis::Vertical),
            Size::new(40.0, 0.0)
        );
    }

    #[test]
    fn measure_stack_uses_shared_remaining_available_logic() {
        let node = row(vec![
            spacer(30.0, 10.0),
            spacer(30.0, 10.0),
            spacer(30.0, 10.0),
        ])
        .padding_all(5.0)
        .spacing(7.0);

        let measured = measure_node(
            &node,
            Point::new(0.0, 0.0),
            Size::new(70.0, 40.0),
            &zeno_text::FallbackTextSystem,
        );

        let MeasuredKind::Multiple(children) = measured.kind else {
            panic!("expected stack children");
        };

        assert_eq!(children[0].frame.size.width, 30.0);
        assert_eq!(children[1].frame.size.width, 23.0);
        assert_eq!(children[2].frame.size.width, 0.0);
    }

    #[test]
    fn arranged_gap_and_offset_centers_stack_content() {
        let (gap, start) = arranged_gap_and_offset(100.0, 30.0, 2, 10.0, crate::Arrangement::Center);
        assert_eq!(gap, 10.0);
        assert_eq!(start, 30.0);
    }
}
