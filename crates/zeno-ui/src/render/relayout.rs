//! relayout 逻辑拆分后更容易单独优化局部重排策略。

use super::fragments::main_axis_extent;
use super::*;

pub(super) fn relayout_node(
    node: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    retained: &RetainedComposeTree,
    layout_dirty_roots: &HashSet<NodeId>,
    force_relayout: bool,
) -> (MeasuredNode, bool) {
    let dirty = layout_dirty_roots.contains(&node.id());
    let descendant_dirty = retained.has_descendant_in(node.id(), layout_dirty_roots);
    let dirty_flags = retained.dirty_flags_for(node.id());
    if !force_relayout && !dirty && !descendant_dirty {
        if let (Some(measured), Some(cached_available)) = (
            retained.measured_for(node.id()),
            retained.available_for(node.id()),
        ) {
            if cached_available == available {
                if measured.frame.origin == origin {
                    return (measured.clone(), true);
                }
                return (translate_measured_node(measured, origin), true);
            }
        }
    }

    match &node.kind {
        NodeKind::Text(text) => {
            let measured = crate::layout::measure_text(node, text, origin, available, text_system);
            let _ = classify_leaf_relayout(retained.measured_for(node.id()), &measured);
            (measured, false)
        }
        NodeKind::Spacer(spacer) => {
            let measured = crate::layout::measure_spacer(node, spacer, origin, available);
            let _ = classify_leaf_relayout(retained.measured_for(node.id()), &measured);
            (measured, false)
        }
        NodeKind::Container(child) => {
            let style = node.resolved_style();
            let previous_measured = retained.measured_for(node.id());
            let previous_child = previous_measured.and_then(|measured| match &measured.kind {
                MeasuredKind::Single(child) => Some(child.as_ref()),
                _ => None,
            });
            let child_available = crate::layout::content_available(node, available);
            let (measured_child, child_reused) = relayout_node(
                child,
                Point::new(origin.x + style.padding.left, origin.y + style.padding.top),
                child_available,
                text_system,
                retained,
                layout_dirty_roots,
                force_relayout || dirty,
            );
            let size = crate::layout::finalize_size(node, available, measured_child.frame.size);
            let measured = MeasuredNode {
                frame: zeno_core::Rect::new(origin.x, origin.y, size.width, size.height),
                kind: MeasuredKind::Single(Box::new(measured_child)),
            };
            let _ = classify_container_relayout(previous_measured, previous_child, &measured);
            let _ = child_reused;
            (measured, false)
        }
        NodeKind::Stack { axis, children } => {
            let child_force_relayout =
                force_relayout || (dirty && !dirty_flags.requires_order_only());
            let measured = relayout_stack(
                node,
                *axis,
                children,
                origin,
                available,
                text_system,
                retained,
                layout_dirty_roots,
                child_force_relayout,
            );
            (measured, false)
        }
    }
}

fn relayout_stack(
    node: &Node,
    axis: crate::Axis,
    children: &[Node],
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    retained: &RetainedComposeTree,
    layout_dirty_roots: &HashSet<NodeId>,
    force_relayout: bool,
) -> MeasuredNode {
    let style = node.resolved_style();
    let mut measured_children = Vec::with_capacity(children.len());
    let content_origin = Point::new(origin.x + style.padding.left, origin.y + style.padding.top);
    let content_available = crate::layout::content_available(node, available);
    let mut cursor = content_origin;
    let mut used_main = 0.0f32;
    let mut max_cross = 0.0f32;
    let spacing = style.spacing;
    let mut downstream_relayout = force_relayout;

    for child in children {
        let remaining =
            crate::layout::remaining_available_for_axis(content_available, used_main, axis);
        let (measured_child, reused) = relayout_node(
            child,
            cursor,
            remaining,
            text_system,
            retained,
            layout_dirty_roots,
            downstream_relayout,
        );
        if !downstream_relayout && !reused {
            let previous_measured = retained.measured_for(child.id());
            let child_class =
                classify_stack_child_relayout(previous_measured, &measured_child, axis);
            downstream_relayout = matches!(child_class, RelayoutClass::ParentAndFollowingSiblings);
        }

        let child_size = measured_child.frame.size;
        match axis {
            crate::Axis::Horizontal => {
                used_main += child_size.width;
                if !measured_children.is_empty() {
                    used_main += spacing;
                }
                cursor.x += child_size.width + spacing;
                max_cross = max_cross.max(child_size.height);
            }
            crate::Axis::Vertical => {
                used_main += child_size.height;
                if !measured_children.is_empty() {
                    used_main += spacing;
                }
                cursor.y += child_size.height + spacing;
                max_cross = max_cross.max(child_size.width);
            }
        }
        measured_children.push(measured_child);
    }

    let content_size = match axis {
        crate::Axis::Horizontal => Size::new(used_main.max(0.0), max_cross),
        crate::Axis::Vertical => Size::new(max_cross, used_main.max(0.0)),
    };
    let final_size = crate::layout::finalize_size(node, available, content_size);
    MeasuredNode {
        frame: zeno_core::Rect::new(origin.x, origin.y, final_size.width, final_size.height),
        kind: MeasuredKind::Multiple(measured_children),
    }
}

fn translate_measured_node(measured: &MeasuredNode, origin: Point) -> MeasuredNode {
    let dx = origin.x - measured.frame.origin.x;
    let dy = origin.y - measured.frame.origin.y;
    translate_measured_node_by_delta(measured, dx, dy)
}

fn translate_measured_node_by_delta(measured: &MeasuredNode, dx: f32, dy: f32) -> MeasuredNode {
    let frame = zeno_core::Rect::new(
        measured.frame.origin.x + dx,
        measured.frame.origin.y + dy,
        measured.frame.size.width,
        measured.frame.size.height,
    );
    let kind = match &measured.kind {
        MeasuredKind::Text(layout) => MeasuredKind::Text(layout.clone()),
        MeasuredKind::Single(child) => {
            MeasuredKind::Single(Box::new(translate_measured_node_by_delta(child, dx, dy)))
        }
        MeasuredKind::Multiple(children) => MeasuredKind::Multiple(
            children
                .iter()
                .map(|child| translate_measured_node_by_delta(child, dx, dy))
                .collect(),
        ),
        MeasuredKind::Spacer => MeasuredKind::Spacer,
    };
    MeasuredNode { frame, kind }
}

fn classify_leaf_relayout(
    previous: Option<&MeasuredNode>,
    current: &MeasuredNode,
) -> RelayoutClass {
    match previous {
        Some(previous) if previous.frame == current.frame => RelayoutClass::LocalOnly,
        Some(_) => RelayoutClass::ParentOnly,
        None => RelayoutClass::ParentOnly,
    }
}

fn classify_container_relayout(
    previous: Option<&MeasuredNode>,
    previous_child: Option<&MeasuredNode>,
    current: &MeasuredNode,
) -> RelayoutClass {
    let Some(previous) = previous else {
        return RelayoutClass::ParentOnly;
    };
    if previous.frame != current.frame {
        return RelayoutClass::ParentOnly;
    }
    let current_child = match &current.kind {
        MeasuredKind::Single(child) => Some(child.as_ref()),
        _ => None,
    };
    if previous_child == current_child {
        RelayoutClass::Reused
    } else {
        RelayoutClass::LocalOnly
    }
}

fn classify_stack_child_relayout(
    previous: Option<&MeasuredNode>,
    current: &MeasuredNode,
    axis: crate::Axis,
) -> RelayoutClass {
    let Some(previous) = previous else {
        return RelayoutClass::ParentAndFollowingSiblings;
    };
    let previous_main = main_axis_extent(previous.frame.size, axis);
    let current_main = main_axis_extent(current.frame.size, axis);
    if (previous_main - current_main).abs() > f32::EPSILON {
        RelayoutClass::ParentAndFollowingSiblings
    } else if previous.frame == current.frame {
        RelayoutClass::LocalOnly
    } else {
        RelayoutClass::ParentOnly
    }
}
