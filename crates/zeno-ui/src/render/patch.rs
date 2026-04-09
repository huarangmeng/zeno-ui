//! patch 模块拆成子模块，分别处理遍历、diff 与 relayout 更新面。

mod collect;
mod diff;
mod update;

use super::*;
use crate::render::fragments::find_node;

pub(super) fn repaint_dirty_nodes(root: &Node, retained: &mut RetainedComposeTree) {
    let dirty_node_ids = retained.dirty_node_ids();
    for node_id in dirty_node_ids {
        if let (Some(node), Some(measured)) = (
            find_node(root, node_id),
            retained.measured_for(node_id).cloned(),
        ) {
            retained.update_fragment(
                node_id,
                crate::render::fragments::node_fragment(node, &measured),
            );
        }
    }
}

pub(super) fn update_fragments_for_nodes(
    node: &Node,
    measured: &MeasuredNode,
    available: Size,
    update_ids: &HashSet<NodeId>,
    retained: &mut RetainedComposeTree,
) -> bool {
    update::update_fragments_for_nodes(node, measured, available, update_ids, retained)
}

pub(super) fn scene_update_ids_for_relayout(
    node: &Node,
    measured: &MeasuredNode,
    retained: &RetainedComposeTree,
    fragment_update_ids: &HashSet<NodeId>,
) -> HashSet<NodeId> {
    update::scene_update_ids_for_relayout(node, measured, retained, fragment_update_ids)
}

pub(super) fn patch_scene_for_nodes(
    root: &Node,
    retained: &mut RetainedComposeTree,
    update_ids: &HashSet<NodeId>,
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
        retained.measured(),
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
    let patch = ScenePatch {
        size: previous_scene.size,
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
