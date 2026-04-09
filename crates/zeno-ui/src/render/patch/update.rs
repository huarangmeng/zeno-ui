//! patch 前的片段更新与 relayout 影响面计算。

use super::*;
use crate::layout::LayoutArena;
use crate::render::fragments::{child_axis, main_axis_extent, node_fragment};

pub(super) fn update_fragments_for_nodes(
    node: &Node,
    layout: &LayoutArena,
    available: Size,
    update_ids: &HashSet<NodeId>,
    retained: &mut RetainedComposeTree,
) -> bool {
    let mut touched = update_ids.contains(&node.id());
    match &node.kind {
        NodeKind::Container(child) => {
            touched |= update_fragments_for_nodes(
                child,
                layout,
                crate::layout::content_available(node, available),
                update_ids,
                retained,
            );
        }
        NodeKind::Box { children } => {
            let child_available = crate::layout::content_available(node, available);
            for child in children {
                touched |= update_fragments_for_nodes(
                    child,
                    layout,
                    child_available,
                    update_ids,
                    retained,
                );
            }
        }
        NodeKind::Stack { children, .. } => {
            let content_available = crate::layout::content_available(node, available);
            let mut used_main = 0.0f32;
            let axis = child_axis(node);
            for (index, child) in children.iter().enumerate() {
                let child_available =
                    crate::layout::remaining_available_for_axis(content_available, used_main, axis);
                touched |= update_fragments_for_nodes(
                    child,
                    layout,
                    child_available,
                    update_ids,
                    retained,
                );
                let child_frame = layout
                    .frame(child.id())
                    .expect("layout frame should exist for child");
                used_main += main_axis_extent(child_frame.size, axis);
                if index + 1 != children.len() {
                    used_main += node.resolved_style().spacing;
                }
            }
        }
        _ => {}
    }
    if touched && update_ids.contains(&node.id()) {
        let slot = layout
            .slot(node.id())
            .expect("layout slot should exist for node fragment");
        retained.update_fragment(node.id(), node_fragment(node, slot, layout));
    }
    touched
}

pub(super) fn scene_update_ids_for_relayout(
    node: &Node,
    layout: &LayoutArena,
    retained: &RetainedComposeTree,
    fragment_update_ids: &HashSet<NodeId>,
) -> HashSet<NodeId> {
    let mut update_ids = HashSet::new();
    collect_scene_update_ids_for_relayout(
        node,
        layout,
        retained,
        fragment_update_ids,
        &mut update_ids,
    );
    update_ids
}

fn collect_scene_update_ids_for_relayout(
    node: &Node,
    layout: &LayoutArena,
    retained: &RetainedComposeTree,
    fragment_update_ids: &HashSet<NodeId>,
    update_ids: &mut HashSet<NodeId>,
) -> bool {
    let current_frame = layout
        .frame(node.id())
        .expect("layout frame should exist for scene update");
    let mut changed = fragment_update_ids.contains(&node.id())
        || retained
            .layout_for(node.id())
            .map_or(true, |previous| previous.frame != current_frame);
    match &node.kind {
        NodeKind::Container(child) => {
            changed |= collect_scene_update_ids_for_relayout(
                child,
                layout,
                retained,
                fragment_update_ids,
                update_ids,
            );
        }
        NodeKind::Box { children } => {
            for child in children {
                changed |= collect_scene_update_ids_for_relayout(
                    child,
                    layout,
                    retained,
                    fragment_update_ids,
                    update_ids,
                );
            }
        }
        NodeKind::Stack { children, .. } => {
            for child in children {
                changed |= collect_scene_update_ids_for_relayout(
                    child,
                    layout,
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
