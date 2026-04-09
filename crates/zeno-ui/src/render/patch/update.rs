//! patch 前的片段更新与 relayout 影响面计算。

use super::*;
use crate::render::fragments::{child_axis, main_axis_extent, node_fragment};

pub(super) fn update_fragments_for_nodes(
    node: &Node,
    measured: &MeasuredNode,
    available: Size,
    update_ids: &HashSet<NodeId>,
    retained: &mut RetainedComposeTree,
) -> bool {
    let mut touched = update_ids.contains(&node.id());
    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            touched |= update_fragments_for_nodes(
                child,
                measured_child,
                crate::layout::content_available(node, available),
                update_ids,
                retained,
            );
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            let content_available = crate::layout::content_available(node, available);
            let mut used_main = 0.0f32;
            let axis = child_axis(node);
            for (index, (child, measured_child)) in
                children.iter().zip(measured_children.iter()).enumerate()
            {
                let child_available =
                    crate::layout::remaining_available_for_axis(content_available, used_main, axis);
                touched |= update_fragments_for_nodes(
                    child,
                    measured_child,
                    child_available,
                    update_ids,
                    retained,
                );
                used_main += main_axis_extent(measured_child.frame.size, axis);
                if index + 1 != children.len() {
                    used_main += node.resolved_style().spacing;
                }
            }
        }
        _ => {}
    }
    if touched && update_ids.contains(&node.id()) {
        retained.update_fragment(node.id(), node_fragment(node, measured));
    }
    touched
}

pub(super) fn scene_update_ids_for_relayout(
    node: &Node,
    measured: &MeasuredNode,
    retained: &RetainedComposeTree,
    fragment_update_ids: &HashSet<NodeId>,
) -> HashSet<NodeId> {
    let mut update_ids = HashSet::new();
    collect_scene_update_ids_for_relayout(
        node,
        measured,
        retained,
        fragment_update_ids,
        &mut update_ids,
    );
    update_ids
}

fn collect_scene_update_ids_for_relayout(
    node: &Node,
    measured: &MeasuredNode,
    retained: &RetainedComposeTree,
    fragment_update_ids: &HashSet<NodeId>,
    update_ids: &mut HashSet<NodeId>,
) -> bool {
    let mut changed = fragment_update_ids.contains(&node.id())
        || retained
            .measured_for(node.id())
            .map_or(true, |previous| previous.frame != measured.frame);
    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            changed |= collect_scene_update_ids_for_relayout(
                child,
                measured_child,
                retained,
                fragment_update_ids,
                update_ids,
            );
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            for (child, measured_child) in children.iter().zip(measured_children.iter()) {
                changed |= collect_scene_update_ids_for_relayout(
                    child,
                    measured_child,
                    retained,
                    fragment_update_ids,
                    update_ids,
                );
            }
        }
        _ => {}
    }
    if changed {
        update_ids.insert(node.id());
    }
    changed
}
