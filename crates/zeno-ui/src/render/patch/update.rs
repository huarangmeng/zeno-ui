//! patch 前的片段更新与 relayout 影响面计算。

use super::*;
use crate::layout::LayoutArena;
use crate::render::fragments::node_fragment;

pub(super) fn update_fragments_for_nodes(
    node: &Node,
    index: usize,
    layout: &LayoutArena,
    available: Size,
    update_ids: &HashSet<usize>,
    retained: &mut RetainedComposeTree,
) -> bool {
    let _ = (node, index, available);
    for &update_index in update_ids {
        let object = retained.objects().object(update_index).clone();
        let slot = layout.slot_at(update_index).clone();
        retained.update_fragment(object.node_id, node_fragment(&object, &slot));
    }
    !update_ids.is_empty()
}

pub(super) fn scene_update_ids_for_relayout(
    node: &Node,
    index: usize,
    layout: &LayoutArena,
    retained: &RetainedComposeTree,
    fragment_update_ids: &HashSet<usize>,
) -> HashSet<usize> {
    let _ = (node, index);
    let mut update_ids = fragment_update_ids.clone();
    for current_index in 0..layout.object_table().len() {
        let object = retained.objects().object(current_index);
        let current_frame = layout.slot_at(current_index).frame;
        let changed = fragment_update_ids.contains(&current_index)
            || retained
                .layout_for(object.node_id)
                .map_or(true, |previous| previous.frame != current_frame);
        if changed {
            update_ids.insert(current_index);
            let mut parent = retained.parent_index_of(current_index);
            while let Some(parent_index) = parent {
                update_ids.insert(parent_index);
                parent = retained.parent_index_of(parent_index);
            }
        }
    }
    update_ids
}
