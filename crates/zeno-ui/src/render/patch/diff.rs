//! patch diff 层只负责判断是 upsert 还是 order-only 更新。

use super::*;

pub(super) fn push_layer_patch(
    previous: &LayerObject,
    current: &LayerObject,
    layer_upserts: &mut Vec<LayerObject>,
    layer_reorders: &mut Vec<LayerOrder>,
) {
    if previous == current {
        return;
    }
    if previous.order != current.order {
        layer_reorders.push(LayerOrder {
            layer_id: current.layer_id,
            order: current.order,
        });
    }
    if !layer_equal_except_order(previous, current) {
        layer_upserts.push(current.clone());
    }
}

pub(super) fn push_block_patch(
    previous: &RenderObject,
    current: &RenderObject,
    upserts: &mut Vec<RenderObject>,
    reorders: &mut Vec<RenderObjectOrder>,
) {
    if previous == current {
        return;
    }
    if previous.order != current.order {
        reorders.push(RenderObjectOrder {
            object_id: current.object_id,
            order: current.order,
        });
    }
    if !block_equal_except_order(previous, current) {
        upserts.push(current.clone());
    }
}

use crate::frontend::FrontendObjectTable;

pub(super) fn subtree_contains_updates(
    objects: &FrontendObjectTable,
    index: usize,
    update_ids: &HashSet<usize>,
) -> bool {
    if update_ids.contains(&index) {
        return true;
    }
    let mut stack = objects.child_indices(index).to_vec();
    while let Some(child_index) = stack.pop() {
        if update_ids.contains(&child_index) {
            return true;
        }
        for &nested in objects.child_indices(child_index) {
            stack.push(nested);
        }
    }
    false
}

pub(super) fn layer_context_changed(
    previous: Option<&LayerObject>,
    current: &LayerObject,
) -> bool {
    previous.map_or(true, |previous| {
        previous.parent_layer_id != current.parent_layer_id
            || previous.transform != current.transform
    })
}

fn layer_equal_except_order(
    previous: &LayerObject,
    current: &LayerObject,
) -> bool {
    previous.layer_id == current.layer_id
        && previous.owner_object_id == current.owner_object_id
        && previous.parent_layer_id == current.parent_layer_id
        && previous.local_bounds == current.local_bounds
        && previous.bounds == current.bounds
        && previous.transform == current.transform
        && previous.clip == current.clip
        && previous.opacity == current.opacity
        && previous.blend_mode == current.blend_mode
        && previous.effects == current.effects
        && previous.offscreen == current.offscreen
}

fn block_equal_except_order(previous: &RenderObject, current: &RenderObject) -> bool {
    previous.object_id == current.object_id
        && previous.layer_id == current.layer_id
        && previous.bounds == current.bounds
        && previous.transform == current.transform
        && previous.clip == current.clip
        && previous.packet_count == current.packet_count
        && previous.packet_signature == current.packet_signature
        && previous.resource_keys == current.resource_keys
}
