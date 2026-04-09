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
                container_child_available(node, available),
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
                container_child_available(node, available),
                available_by_node,
            );
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            let content_available = container_child_available(node, available);
            let mut used_main = 0.0f32;
            for (index, (child, measured_child)) in
                children.iter().zip(measured_children.iter()).enumerate()
            {
                let child_available = match child_axis(node) {
                    crate::Axis::Horizontal => Size::new(
                        (content_available.width - used_main).max(0.0),
                        content_available.height,
                    ),
                    crate::Axis::Vertical => Size::new(
                        content_available.width,
                        (content_available.height - used_main).max(0.0),
                    ),
                };
                collect_available(child, measured_child, child_available, available_by_node);
                used_main += main_axis_extent(measured_child.frame.size, child_axis(node));
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
    let content_available = container_child_available(node, available);
    let mut used_main = 0.0f32;
    for (index, (child, measured_child)) in
        children.iter().zip(measured_children.iter()).enumerate()
    {
        let child_available = match child_axis(node) {
            crate::Axis::Horizontal => Size::new(
                (content_available.width - used_main).max(0.0),
                content_available.height,
            ),
            crate::Axis::Vertical => Size::new(
                content_available.width,
                (content_available.height - used_main).max(0.0),
            ),
        };
        collect_fragments(
            child,
            measured_child,
            child_available,
            available_by_node,
            fragments_by_node,
        );
        let child_size = measured_child.frame.size;
        match child_axis(node) {
            crate::Axis::Horizontal => used_main += child_size.width,
            crate::Axis::Vertical => used_main += child_size.height,
        }
        if index + 1 != children.len() {
            used_main += node.resolved_style().spacing;
        }
    }
}

pub(super) fn container_child_available(node: &Node, available: Size) -> Size {
    let style = node.resolved_style();
    Size::new(
        (available.width - style.padding.left - style.padding.right).max(0.0),
        (available.height - style.padding.top - style.padding.bottom).max(0.0),
    )
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
