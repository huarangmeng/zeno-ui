//! 片段与 available map 的构建拆出来，便于后续单独优化缓存命中策略。

use super::scene::build_scene;
use super::*;

pub(super) fn structured_scene_from_measured(
    root: &Node,
    viewport: Size,
    measured: &MeasuredNode,
) -> (
    HashMap<NodeId, Size>,
    HashMap<NodeId, Vec<DrawCommand>>,
    Scene,
) {
    let mut fragments_by_node = HashMap::new();
    let mut available_by_node = HashMap::new();
    collect_fragments(
        root,
        measured,
        viewport,
        &mut available_by_node,
        &mut fragments_by_node,
    );
    let scene = build_scene(root, measured, viewport, &fragments_by_node);
    (available_by_node, fragments_by_node, scene)
}

pub(super) fn available_map_from_measured(
    root: &Node,
    viewport: Size,
    measured: &MeasuredNode,
) -> HashMap<NodeId, Size> {
    let mut available_by_node = HashMap::new();
    collect_available(root, measured, viewport, &mut available_by_node);
    available_by_node
}

pub(super) fn collect_fragments(
    node: &Node,
    measured: &MeasuredNode,
    available: Size,
    available_by_node: &mut HashMap<NodeId, Size>,
    fragments_by_node: &mut HashMap<NodeId, Vec<DrawCommand>>,
) {
    available_by_node.insert(node.id(), available);
    fragments_by_node.insert(node.id(), node_fragment(node, measured));

    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            collect_fragments(
                child,
                measured_child,
                crate::layout::content_available(node, available),
                available_by_node,
                fragments_by_node,
            );
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            collect_stack_fragments(
                node,
                children,
                measured_children,
                available,
                available_by_node,
                fragments_by_node,
            );
        }
        _ => {}
    }
}

fn collect_available(
    node: &Node,
    measured: &MeasuredNode,
    available: Size,
    available_by_node: &mut HashMap<NodeId, Size>,
) {
    available_by_node.insert(node.id(), available);
    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            collect_available(
                child,
                measured_child,
                crate::layout::content_available(node, available),
                available_by_node,
            );
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            let content_available = crate::layout::content_available(node, available);
            let axis = child_axis(node);
            let mut used_main = 0.0f32;
            for (index, (child, measured_child)) in
                children.iter().zip(measured_children.iter()).enumerate()
            {
                let child_available =
                    crate::layout::remaining_available_for_axis(content_available, used_main, axis);
                collect_available(child, measured_child, child_available, available_by_node);
                used_main += main_axis_extent(measured_child.frame.size, axis);
                if index + 1 != children.len() {
                    used_main += node.resolved_style().spacing;
                }
            }
        }
        _ => {}
    }
}

pub(super) fn node_fragment(node: &Node, measured: &MeasuredNode) -> Vec<DrawCommand> {
    let style = node.resolved_style();
    let mut fragment = Vec::new();
    let local_bounds = Rect::new(
        0.0,
        0.0,
        measured.frame.size.width,
        measured.frame.size.height,
    );
    if let Some(background) = style.background {
        let shape = if style.corner_radius > 0.0 {
            Shape::RoundedRect {
                rect: local_bounds,
                radius: style.corner_radius,
            }
        } else {
            Shape::Rect(local_bounds)
        };
        fragment.push(DrawCommand::Fill {
            shape,
            brush: Brush::Solid(background),
        });
    }

    if let (NodeKind::Text(_), MeasuredKind::Text(layout)) = (&node.kind, &measured.kind) {
        let position = Point::new(
            style.padding.left,
            style.padding.top + layout.metrics.ascent,
        );
        fragment.push(DrawCommand::Text {
            position,
            layout: layout.clone(),
            color: style.foreground,
        });
    }

    fragment
}

pub(super) fn find_node(node: &Node, node_id: NodeId) -> Option<&Node> {
    if node.id() == node_id {
        return Some(node);
    }

    match &node.kind {
        NodeKind::Container(child) => find_node(child, node_id),
        NodeKind::Stack { children, .. } => {
            children.iter().find_map(|child| find_node(child, node_id))
        }
        _ => None,
    }
}

fn collect_stack_fragments(
    node: &Node,
    children: &[Node],
    measured_children: &[MeasuredNode],
    available: Size,
    available_by_node: &mut HashMap<NodeId, Size>,
    fragments_by_node: &mut HashMap<NodeId, Vec<DrawCommand>>,
) {
    let content_available = crate::layout::content_available(node, available);
    let mut used_main = 0.0f32;
    let axis = child_axis(node);
    for (index, (child, measured_child)) in
        children.iter().zip(measured_children.iter()).enumerate()
    {
        let child_available =
            crate::layout::remaining_available_for_axis(content_available, used_main, axis);
        collect_fragments(
            child,
            measured_child,
            child_available,
            available_by_node,
            fragments_by_node,
        );
        used_main += main_axis_extent(measured_child.frame.size, axis);
        if index + 1 != children.len() {
            used_main += node.resolved_style().spacing;
        }
    }
}

pub(super) fn child_axis(node: &Node) -> crate::Axis {
    match &node.kind {
        NodeKind::Stack { axis, .. } => *axis,
        _ => crate::Axis::Vertical,
    }
}

pub(super) fn main_axis_extent(size: Size, axis: crate::Axis) -> f32 {
    match axis {
        crate::Axis::Horizontal => size.width,
        crate::Axis::Vertical => size.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{row, spacer};
    use zeno_text::FallbackTextSystem;

    #[test]
    fn available_map_uses_shared_available_helpers_for_stack_children() {
        let first = spacer(30.0, 10.0);
        let second = spacer(30.0, 10.0);
        let third = spacer(30.0, 10.0);
        let first_id = first.id();
        let second_id = second.id();
        let third_id = third.id();
        let root = row(vec![first, second, third])
            .padding_all(5.0)
            .spacing(7.0);
        let viewport = Size::new(70.0, 40.0);

        let measured =
            crate::layout::measure_node(&root, Point::new(0.0, 0.0), viewport, &FallbackTextSystem);
        let available_by_node = available_map_from_measured(&root, viewport, &measured);

        assert_eq!(available_by_node[&root.id()], viewport);
        assert_eq!(available_by_node[&first_id], Size::new(60.0, 30.0));
        assert_eq!(available_by_node[&second_id], Size::new(23.0, 30.0));
        assert_eq!(available_by_node[&third_id], Size::new(0.0, 30.0));
    }
}
