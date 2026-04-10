//! patch 前的片段更新与 relayout 影响面计算。

use super::*;
use crate::layout::LayoutArena;
use crate::render::fragments::{child_axis, main_axis_extent, node_fragment};

pub(super) fn update_fragments_for_nodes(
    node: &Node,
    index: usize,
    layout: &LayoutArena,
    available: Size,
    update_ids: &HashSet<usize>,
    retained: &mut RetainedComposeTree,
) -> bool {
    let mut touched = update_ids.contains(&index);
    match &node.kind {
        NodeKind::Container(child) => {
            let child_index = layout.index_table().child_indices(index)[0];
            touched |= update_fragments_for_nodes(
                child,
                child_index,
                layout,
                crate::layout::content_available(node, available),
                update_ids,
                retained,
            );
        }
        NodeKind::Box { children } => {
            let child_available = crate::layout::content_available(node, available);
            for (child, child_index) in children
                .iter()
                .zip(layout.index_table().child_indices(index).iter().copied())
            {
                touched |= update_fragments_for_nodes(
                    child,
                    child_index,
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
            for (position, (child, child_index)) in children
                .iter()
                .zip(layout.index_table().child_indices(index).iter().copied())
                .enumerate()
            {
                let child_available =
                    crate::layout::remaining_available_for_axis(content_available, used_main, axis);
                touched |= update_fragments_for_nodes(
                    child,
                    child_index,
                    layout,
                    child_available,
                    update_ids,
                    retained,
                );
                let child_frame = layout
                    .frame(child.id())
                    .expect("layout frame should exist for child");
                used_main += main_axis_extent(child_frame.size, axis);
                if position + 1 != children.len() {
                    used_main += node.resolved_style().spacing;
                }
            }
        }
        _ => {}
    }
    if touched && update_ids.contains(&index) {
        let slot = layout
            .slot(node.id())
            .expect("layout slot should exist for node fragment");
        retained.update_fragment(node.id(), node_fragment(node, slot, layout));
    }
    touched
}

pub(super) fn scene_update_ids_for_relayout(
    node: &Node,
    index: usize,
    layout: &LayoutArena,
    retained: &RetainedComposeTree,
    fragment_update_ids: &HashSet<usize>,
) -> HashSet<usize> {
    let mut update_ids = HashSet::new();
    collect_scene_update_ids_for_relayout(
        node,
        index,
        layout,
        retained,
        fragment_update_ids,
        &mut update_ids,
    );
    update_ids
}

fn collect_scene_update_ids_for_relayout(
    node: &Node,
    index: usize,
    layout: &LayoutArena,
    retained: &RetainedComposeTree,
    fragment_update_ids: &HashSet<usize>,
    update_ids: &mut HashSet<usize>,
) -> bool {
    let current_frame = layout.slot_at(index).frame;
    let mut changed = fragment_update_ids.contains(&index)
        || retained
            .layout_for(node.id())
            .map_or(true, |previous| previous.frame != current_frame);
    match &node.kind {
        NodeKind::Container(child) => {
            let child_index = layout.index_table().child_indices(index)[0];
            changed |= collect_scene_update_ids_for_relayout(
                child,
                child_index,
                layout,
                retained,
                fragment_update_ids,
                update_ids,
            );
        }
        NodeKind::Box { children } => {
            for (child, child_index) in children
                .iter()
                .zip(layout.index_table().child_indices(index).iter().copied())
            {
                changed |= collect_scene_update_ids_for_relayout(
                    child,
                    child_index,
                    layout,
                    retained,
                    fragment_update_ids,
                    update_ids,
                );
            }
        }
        NodeKind::Stack { children, .. } => {
            for (child, child_index) in children
                .iter()
                .zip(layout.index_table().child_indices(index).iter().copied())
            {
                changed |= collect_scene_update_ids_for_relayout(
                    child,
                    child_index,
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
        update_ids.insert(index);
    }
    changed
}
