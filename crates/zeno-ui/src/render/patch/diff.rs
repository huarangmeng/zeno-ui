//! patch diff 层只负责判断是 upsert 还是 order-only 更新。

use super::*;

pub(super) fn push_layer_patch(
    previous: &zeno_scene::SceneLayer,
    current: &zeno_scene::SceneLayer,
    layer_upserts: &mut Vec<zeno_scene::SceneLayer>,
    layer_reorders: &mut Vec<SceneLayerOrder>,
) {
    if previous == current {
        return;
    }
    if previous.order != current.order {
        layer_reorders.push(SceneLayerOrder {
            layer_id: current.layer_id,
            order: current.order,
        });
    }
    if !layer_equal_except_order(previous, current) {
        layer_upserts.push(current.clone());
    }
}

pub(super) fn push_block_patch(
    previous: &SceneBlock,
    current: &SceneBlock,
    upserts: &mut Vec<SceneBlock>,
    reorders: &mut Vec<SceneBlockOrder>,
) {
    if previous == current {
        return;
    }
    if previous.order != current.order {
        reorders.push(SceneBlockOrder {
            node_id: current.node_id,
            order: current.order,
        });
    }
    if !block_equal_except_order(previous, current) {
        upserts.push(current.clone());
    }
}

pub(super) fn subtree_contains_updates(
    node: &Node,
    index: usize,
    layout: &crate::layout::LayoutArena,
    update_ids: &HashSet<usize>,
) -> bool {
    if update_ids.contains(&index) {
        return true;
    }
    match &node.kind {
        NodeKind::Container(child) => subtree_contains_updates(
            child,
            layout.index_table().child_indices(index)[0],
            layout,
            update_ids,
        ),
        NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
            children.iter().zip(layout.index_table().child_indices(index)).any(
                |(child, child_index)| subtree_contains_updates(child, *child_index, layout, update_ids),
            )
        }
        _ => false,
    }
}

pub(super) fn layer_context_changed(
    previous: Option<&zeno_scene::SceneLayer>,
    current: &zeno_scene::SceneLayer,
) -> bool {
    previous.map_or(true, |previous| {
        previous.parent_layer_id != current.parent_layer_id
            || previous.transform != current.transform
    })
}

fn layer_equal_except_order(
    previous: &zeno_scene::SceneLayer,
    current: &zeno_scene::SceneLayer,
) -> bool {
    previous.layer_id == current.layer_id
        && previous.node_id == current.node_id
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

fn block_equal_except_order(previous: &SceneBlock, current: &SceneBlock) -> bool {
    previous.node_id == current.node_id
        && previous.layer_id == current.layer_id
        && previous.bounds == current.bounds
        && previous.transform == current.transform
        && previous.clip == current.clip
        && previous.command_count == current.command_count
        && previous.command_signature == current.command_signature
        && previous.resource_keys == current.resource_keys
}
