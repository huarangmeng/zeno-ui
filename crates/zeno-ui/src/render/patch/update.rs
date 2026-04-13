//! patch 前的片段更新与 relayout 影响面计算。

use super::*;
use crate::layout::LayoutArena;

pub(super) fn update_fragments_for_nodes(
    node: &Node,
    index: usize,
    layout: &LayoutArena,
    available: Size,
    update_ids: &HashSet<usize>,
    retained: &mut RetainedComposeTree,
) -> bool {
    let _ = (node, index, available);
    let _ = (layout, retained);
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
    let object_table = layout.object_table();
    for current_index in 0..object_table.len() {
        let object = object_table.object(current_index);
        let current_frame = layout.slot_at(current_index).frame;
        let changed = fragment_update_ids.contains(&current_index)
            || retained
                .layout_for(object.node_id)
                .map_or(true, |previous| previous.frame != current_frame);
        if changed {
            update_ids.insert(current_index);
            let mut parent = object_table.parent_index_of(current_index);
            while let Some(parent_index) = parent {
                update_ids.insert(parent_index);
                parent = object_table.parent_index_of(parent_index);
            }
        }
    }
    update_ids
}
