//! patch 模块拆成子模块，分别处理遍历、diff 与 relayout 更新面。

mod collect;
mod diff;
mod update;

use super::*;
use crate::layout::LayoutArena;

pub(super) fn repaint_dirty_nodes(root: &Node, retained: &mut RetainedComposeTree) {
    let _ = root;
    let dirty_indices = retained.dirty_indices();
    for index in dirty_indices {
        let object = retained.objects().object(index).clone();
        let slot = retained.layout().slot_at(index).clone();
        retained.update_fragment(
            object.node_id,
            crate::render::fragments::node_fragment(&object, &slot),
        );
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
) -> RenderObjectDelta {
    let _ = root;
    let previous_scene = retained.scene().clone();
    let previous_layers_by_id: HashMap<u64, &LayerObject> = previous_scene
        .layer_graph
        .iter()
        .map(|layer| (layer.layer_id, layer))
        .collect();
    let previous_objects_by_id: HashMap<u64, &RenderObject> = previous_scene
        .objects
        .iter()
        .map(|object| (object.object_id, object))
        .collect();
    let mut layer_upserts = Vec::new();
    let mut layer_reorders = Vec::new();
    let mut upserts = Vec::new();
    let mut reorders = Vec::new();
    let mut seen_layers = HashSet::from([Scene::ROOT_LAYER_ID]);
    let mut seen_objects = HashSet::new();
    let mut next_order = 1u32;
    collect::collect_scene_patch_items(
        retained.objects(),
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
        &previous_objects_by_id,
        &mut seen_layers,
        &mut seen_objects,
        &mut layer_upserts,
        &mut layer_reorders,
        &mut upserts,
        &mut reorders,
    );
    let layer_removes = previous_scene
        .layer_graph
        .iter()
        .filter(|layer| layer.layer_id != Scene::ROOT_LAYER_ID)
        .filter(|layer| !seen_layers.contains(&layer.layer_id))
        .map(|layer| layer.layer_id)
        .collect();
    let removes = previous_scene
        .objects
        .iter()
        .filter(|object| !seen_objects.contains(&object.object_id))
        .map(|object| object.object_id)
        .collect();
    let (packets, upserts) = Scene::compact_objects(upserts);
    let patch = RenderObjectDelta {
        size: previous_scene.size,
        packets,
        base_layer_count: previous_scene.layer_graph.len(),
        base_object_count: previous_scene.objects.len(),
        layer_upserts,
        layer_reorders,
        layer_removes,
        object_upserts: upserts,
        object_reorders: reorders,
        object_removes: removes,
    };
    let scene = previous_scene.apply_delta(&patch);
    retained.replace_scene(scene);
    patch
}
