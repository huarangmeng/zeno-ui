//! patch scene 遍历独立拆分，方便继续优化局部提交粒度。

use super::*;
use crate::frontend::FrontendObjectTable;
use crate::layout::LayoutArena;
use crate::render::fragments::node_fragment;
use crate::render::patch::diff::{
    layer_context_changed, push_block_patch, push_layer_patch, subtree_contains_updates,
};
use crate::render::scene::{
    layer_local_transform, node_creates_layer, scene_blend_mode, scene_clip, scene_effect_bounds,
    scene_effects,
};

pub(super) fn collect_scene_patch_items(
    objects: &FrontendObjectTable,
    index: usize,
    layout: &LayoutArena,
    current_layer_id: u64,
    current_layer_origin: Point,
    current_layer_world_transform: Transform2D,
    force_update: bool,
    update_ids: &HashSet<usize>,
    next_order: &mut u32,
    previous_layers_by_id: &HashMap<u64, &LayerObject>,
    previous_blocks_by_id: &HashMap<u64, &RenderObject>,
    seen_layers: &mut HashSet<u64>,
    seen_blocks: &mut HashSet<u64>,
    layer_upserts: &mut Vec<LayerObject>,
    layer_reorders: &mut Vec<LayerOrder>,
    upserts: &mut Vec<RenderObject>,
    reorders: &mut Vec<RenderObjectOrder>,
) {
    let object = objects.object(index);
    let has_subtree_updates = subtree_contains_updates(objects, index, update_ids);
    if !force_update && !update_ids.contains(&index) && !has_subtree_updates {
        collect_unchanged_scene_items(
            objects,
            index,
            layout,
            current_layer_id,
            current_layer_origin,
            current_layer_world_transform,
            next_order,
            previous_layers_by_id,
            previous_blocks_by_id,
            seen_layers,
            seen_blocks,
            layer_reorders,
            reorders,
        );
        return;
    }

    let slot = layout.slot_at(index);
    let style = &object.style;
    let local_bounds = Rect::new(0.0, 0.0, slot.frame.size.width, slot.frame.size.height);

    if node_creates_layer(style) {
        let layer_transform = layer_local_transform(
            slot.frame.origin,
            current_layer_origin,
            slot.frame.size,
            style.transform,
            style.transform_origin,
        );
        let world_transform = current_layer_world_transform.then(layer_transform);
        let effect_bounds = scene_effect_bounds(local_bounds, style);
        let layer_id = object.node_id.0;
        let order = *next_order;
        *next_order += 1;
        let current_layer = LayerObject::new(
            layer_id,
            object.node_id.0,
            Some(current_layer_id),
            order,
            local_bounds,
            world_transform.map_rect(effect_bounds),
            layer_transform,
            scene_clip(slot.frame.size, style.clip),
            style.opacity,
            scene_blend_mode(style.blend_mode),
            scene_effects(style),
            style.layer
                || style.opacity < 1.0
                || style.blend_mode != BlendMode::Normal
                || style.blur.is_some()
                || style.drop_shadow.is_some(),
        );
        let force_descendant_update = force_update
            || layer_context_changed(previous_layers_by_id.get(&layer_id).copied(), &current_layer);
        seen_layers.insert(layer_id);
        if let Some(previous) = previous_layers_by_id.get(&layer_id) {
            push_layer_patch(previous, &current_layer, layer_upserts, layer_reorders);
        } else {
            layer_upserts.push(current_layer.clone());
        }
        let fragment = node_fragment(object, slot);
        if !fragment.is_empty() {
            let current_block = RenderObject::new(
                object.node_id.0,
                layer_id,
                *next_order,
                world_transform.map_rect(local_bounds),
                Transform2D::identity(),
                None,
                fragment,
            );
            seen_blocks.insert(current_block.object_id);
            if let Some(previous) = previous_blocks_by_id.get(&current_block.object_id) {
                push_block_patch(previous, &current_block, upserts, reorders);
            } else {
                upserts.push(current_block.clone());
            }
            *next_order += 1;
        }
        for &child_index in objects.child_indices(index) {
            collect_scene_patch_items(
                objects,
                child_index,
                layout,
                layer_id,
                slot.frame.origin,
                world_transform,
                force_descendant_update,
                update_ids,
                next_order,
                previous_layers_by_id,
                previous_blocks_by_id,
                seen_layers,
                seen_blocks,
                layer_upserts,
                layer_reorders,
                upserts,
                reorders,
            );
        }
        return;
    }

    let force_descendant_update = force_update || previous_layers_by_id.contains_key(&object.node_id.0);
    let fragment = node_fragment(object, slot);
    if !fragment.is_empty() {
        let block_transform = Transform2D::translation(
            slot.frame.origin.x - current_layer_origin.x,
            slot.frame.origin.y - current_layer_origin.y,
        );
        let world_transform = current_layer_world_transform.then(block_transform);
        let current_block = RenderObject::new(
            object.node_id.0,
            current_layer_id,
            *next_order,
            world_transform.map_rect(local_bounds),
            block_transform,
            None,
            fragment,
        );
        seen_blocks.insert(current_block.object_id);
        if let Some(previous) = previous_blocks_by_id.get(&current_block.object_id) {
            push_block_patch(previous, &current_block, upserts, reorders);
        } else {
            upserts.push(current_block.clone());
        }
        *next_order += 1;
    }
    for &child_index in objects.child_indices(index) {
        collect_scene_patch_items(
            objects,
            child_index,
            layout,
            current_layer_id,
            current_layer_origin,
            current_layer_world_transform,
            force_descendant_update,
            update_ids,
            next_order,
            previous_layers_by_id,
            previous_blocks_by_id,
            seen_layers,
            seen_blocks,
            layer_upserts,
            layer_reorders,
            upserts,
            reorders,
        );
    }
}

fn collect_unchanged_scene_items(
    objects: &FrontendObjectTable,
    index: usize,
    layout: &LayoutArena,
    current_layer_id: u64,
    current_layer_origin: Point,
    current_layer_world_transform: Transform2D,
    next_order: &mut u32,
    previous_layers_by_id: &HashMap<u64, &LayerObject>,
    previous_blocks_by_id: &HashMap<u64, &RenderObject>,
    seen_layers: &mut HashSet<u64>,
    seen_blocks: &mut HashSet<u64>,
    layer_reorders: &mut Vec<LayerOrder>,
    reorders: &mut Vec<RenderObjectOrder>,
) {
    let object = objects.object(index);
    let slot = layout.slot_at(index);
    let style = &object.style;
    let local_bounds = Rect::new(0.0, 0.0, slot.frame.size.width, slot.frame.size.height);

    if node_creates_layer(style) {
        let layer_transform = layer_local_transform(
            slot.frame.origin,
            current_layer_origin,
            slot.frame.size,
            style.transform,
            style.transform_origin,
        );
        let world_transform = current_layer_world_transform.then(layer_transform);
        let effect_bounds = scene_effect_bounds(local_bounds, style);
        let current_layer = LayerObject::new(
            object.node_id.0,
            object.node_id.0,
            Some(current_layer_id),
            *next_order,
            local_bounds,
            world_transform.map_rect(effect_bounds),
            layer_transform,
            scene_clip(slot.frame.size, style.clip),
            style.opacity,
            scene_blend_mode(style.blend_mode),
            scene_effects(style),
            style.layer
                || style.opacity < 1.0
                || style.blend_mode != BlendMode::Normal
                || style.blur.is_some()
                || style.drop_shadow.is_some(),
        );
        seen_layers.insert(current_layer.layer_id);
        if let Some(previous) = previous_layers_by_id.get(&current_layer.layer_id) {
            push_layer_patch(previous, &current_layer, &mut Vec::new(), layer_reorders);
        }
        *next_order += 1;
        let fragment = node_fragment(object, slot);
        if !fragment.is_empty() {
            let current_block = RenderObject::new(
                object.node_id.0,
                current_layer.layer_id,
                *next_order,
                world_transform.map_rect(local_bounds),
                Transform2D::identity(),
                None,
                fragment,
            );
            seen_blocks.insert(current_block.object_id);
            if let Some(previous) = previous_blocks_by_id.get(&current_block.object_id) {
                push_block_patch(previous, &current_block, &mut Vec::new(), reorders);
            }
            *next_order += 1;
        }
        for &child_index in objects.child_indices(index) {
            collect_unchanged_scene_items(
                objects,
                child_index,
                layout,
                current_layer.layer_id,
                slot.frame.origin,
                world_transform,
                next_order,
                previous_layers_by_id,
                previous_blocks_by_id,
                seen_layers,
                seen_blocks,
                layer_reorders,
                reorders,
            );
        }
        return;
    }

    let fragment = node_fragment(object, slot);
    if !fragment.is_empty() {
        let block_transform = Transform2D::translation(
            slot.frame.origin.x - current_layer_origin.x,
            slot.frame.origin.y - current_layer_origin.y,
        );
        let world_transform = current_layer_world_transform.then(block_transform);
        let current_block = RenderObject::new(
            object.node_id.0,
            current_layer_id,
            *next_order,
            world_transform.map_rect(local_bounds),
            block_transform,
            None,
            fragment,
        );
        seen_blocks.insert(current_block.object_id);
        if let Some(previous) = previous_blocks_by_id.get(&current_block.object_id) {
            push_block_patch(previous, &current_block, &mut Vec::new(), reorders);
        }
        *next_order += 1;
    }
    for &child_index in objects.child_indices(index) {
        collect_unchanged_scene_items(
            objects,
            child_index,
            layout,
            current_layer_id,
            current_layer_origin,
            current_layer_world_transform,
            next_order,
            previous_layers_by_id,
            previous_blocks_by_id,
            seen_layers,
            seen_blocks,
            layer_reorders,
            reorders,
        );
    }
}
