//! patch 模块拆成子模块，分别处理遍历、diff 与 relayout 更新面。

mod collect;
mod diff;
mod update;

use super::*;
use crate::layout::LayoutArena;

pub(super) fn repaint_dirty_nodes(root: &Node, retained: &mut RetainedComposeTree) {
    let dirty_indices = retained.dirty_indices();
    for index in dirty_indices {
        if let Some(node) = node_at_index(root, 0, index, retained.layout()) {
            let slot = retained.layout().slot_at(index).clone();
            retained.update_fragment(
                retained.layout().index_table().node_id_at(index),
                crate::render::fragments::node_fragment(node, &slot, retained.layout()),
            );
        }
    }
}

fn node_at_index<'a>(
    node: &'a Node,
    current_index: usize,
    target_index: usize,
    layout: &LayoutArena,
) -> Option<&'a Node> {
    if current_index == target_index {
        return Some(node);
    }
    match &node.kind {
        NodeKind::Container(child) => node_at_index(
            child,
            layout.index_table().child_indices(current_index)[0],
            target_index,
            layout,
        ),
        NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
            for (child, child_index) in children
                .iter()
                .zip(layout.index_table().child_indices(current_index).iter().copied())
            {
                if let Some(found) = node_at_index(child, child_index, target_index, layout) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

pub(super) fn update_fragments_for_nodes(
    node: &Node,
    layout: &LayoutArena,
    available: Size,
    update_ids: &HashSet<usize>,
    retained: &mut RetainedComposeTree,
) -> bool {
    update::update_fragments_for_nodes(node, 0, layout, available, update_ids, retained)
}

pub(super) fn scene_update_ids_for_relayout(
    node: &Node,
    layout: &LayoutArena,
    retained: &RetainedComposeTree,
    fragment_update_ids: &HashSet<usize>,
) -> HashSet<usize> {
    update::scene_update_ids_for_relayout(node, 0, layout, retained, fragment_update_ids)
}

pub(super) fn patch_scene_for_nodes(
    root: &Node,
    retained: &mut RetainedComposeTree,
    update_ids: &HashSet<usize>,
) -> ScenePatch {
    let previous_scene = retained.scene().clone();
    let previous_layers_by_id: HashMap<u64, &zeno_scene::SceneLayer> = previous_scene
        .layers
        .iter()
        .map(|layer| (layer.layer_id, layer))
        .collect();
    let previous_blocks_by_id: HashMap<u64, &SceneBlock> = previous_scene
        .blocks
        .iter()
        .map(|block| (block.node_id, block))
        .collect();
    let mut layer_upserts = Vec::new();
    let mut layer_reorders = Vec::new();
    let mut upserts = Vec::new();
    let mut reorders = Vec::new();
    let mut seen_layers = HashSet::from([Scene::ROOT_LAYER_ID]);
    let mut seen_blocks = HashSet::new();
    let mut next_order = 1u32;
    collect::collect_scene_patch_items(
        root,
        0,
        retained.layout(),
        retained.fragments(),
        Scene::ROOT_LAYER_ID,
        Point::new(0.0, 0.0),
        Transform2D::identity(),
        false,
        update_ids,
        &mut next_order,
        &previous_layers_by_id,
        &previous_blocks_by_id,
        &mut seen_layers,
        &mut seen_blocks,
        &mut layer_upserts,
        &mut layer_reorders,
        &mut upserts,
        &mut reorders,
    );
    let layer_removes = previous_scene
        .layers
        .iter()
        .filter(|layer| layer.layer_id != Scene::ROOT_LAYER_ID)
        .filter(|layer| !seen_layers.contains(&layer.layer_id))
        .map(|layer| layer.layer_id)
        .collect();
    let removes = previous_scene
        .blocks
        .iter()
        .filter(|block| !seen_blocks.contains(&block.node_id))
        .map(|block| block.node_id)
        .collect();
    let (commands, upserts) = Scene::compact_blocks(upserts);
    let patch = ScenePatch {
        size: previous_scene.size,
        commands,
        base_layer_count: previous_scene.layers.len(),
        base_block_count: previous_scene.blocks.len(),
        layer_upserts,
        layer_reorders,
        layer_removes,
        upserts,
        reorders,
        removes,
    };
    let scene = previous_scene.apply_patch(&patch);
    retained.replace_scene(scene);
    patch
}
