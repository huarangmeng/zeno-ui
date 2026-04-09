//! patch scene 遍历独立拆分，方便继续优化局部提交粒度。

use super::*;
use crate::render::patch::diff::{
    layer_context_changed, push_block_patch, push_layer_patch, subtree_contains_updates,
};
use crate::render::scene::{
    layer_local_transform, node_creates_layer, scene_blend_mode, scene_clip, scene_effect_bounds,
    scene_effects,
};

pub(super) fn collect_scene_patch_items(
    node: &Node,
    measured: &MeasuredNode,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
    current_layer_id: u64,
    current_layer_origin: Point,
    current_layer_world_transform: Transform2D,
    force_update: bool,
    update_ids: &HashSet<NodeId>,
    next_order: &mut u32,
    previous_layers_by_id: &HashMap<u64, &zeno_graphics::SceneLayer>,
    previous_blocks_by_id: &HashMap<u64, &SceneBlock>,
    seen_layers: &mut HashSet<u64>,
    seen_blocks: &mut HashSet<u64>,
    layer_upserts: &mut Vec<zeno_graphics::SceneLayer>,
    layer_reorders: &mut Vec<SceneLayerOrder>,
    upserts: &mut Vec<SceneBlock>,
    reorders: &mut Vec<SceneBlockOrder>,
) {
    if !force_update
        && !update_ids.contains(&node.id())
        && !subtree_contains_updates(node, update_ids)
    {
        collect_unchanged_scene_items(
            node,
            measured,
            fragments_by_node,
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
    let style = node.resolved_style();
    let local_bounds = Rect::new(
        0.0,
        0.0,
        measured.frame.size.width,
        measured.frame.size.height,
    );
    if node_creates_layer(&style) {
        let layer_transform = layer_local_transform(
            measured.frame.origin,
            current_layer_origin,
            measured.frame.size,
            style.transform,
            style.transform_origin,
        );
        let world_transform = current_layer_world_transform.then(layer_transform);
        let effect_bounds = scene_effect_bounds(local_bounds, &style);
        let layer_id = node.id().0;
        let order = *next_order;
        *next_order += 1;
        let current_layer = zeno_graphics::SceneLayer::new(
            layer_id,
            node.id().0,
            Some(current_layer_id),
            order,
            local_bounds,
            world_transform.map_rect(effect_bounds),
            layer_transform,
            scene_clip(measured.frame.size, style.clip),
            style.opacity,
            scene_blend_mode(style.blend_mode),
            scene_effects(&style),
            style.layer
                || style.opacity < 1.0
                || style.blend_mode != BlendMode::Normal
                || style.blur.is_some()
                || style.drop_shadow.is_some(),
        );
        let force_descendant_update = force_update
            || layer_context_changed(
                previous_layers_by_id.get(&layer_id).copied(),
                &current_layer,
            );
        seen_layers.insert(layer_id);
        if let Some(previous) = previous_layers_by_id.get(&layer_id) {
            push_layer_patch(previous, &current_layer, layer_upserts, layer_reorders);
        } else {
            layer_upserts.push(current_layer.clone());
        }
        if let Some(fragment) = fragments_by_node.get(&node.id()) {
            let current_block = SceneBlock::new(
                node.id().0,
                layer_id,
                *next_order,
                world_transform.map_rect(local_bounds),
                Transform2D::identity(),
                None,
                fragment.clone(),
            );
            seen_blocks.insert(current_block.node_id);
            if let Some(previous) = previous_blocks_by_id.get(&current_block.node_id) {
                push_block_patch(previous, &current_block, upserts, reorders);
            } else {
                upserts.push(current_block.clone());
            }
            *next_order += 1;
        }
        collect_scene_patch_children(
            node,
            measured,
            fragments_by_node,
            layer_id,
            measured.frame.origin,
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
        return;
    }

    let force_descendant_update = force_update || previous_layers_by_id.contains_key(&node.id().0);
    if let Some(fragment) = fragments_by_node.get(&node.id()) {
        let block_transform = Transform2D::translation(
            measured.frame.origin.x - current_layer_origin.x,
            measured.frame.origin.y - current_layer_origin.y,
        );
        let world_transform = current_layer_world_transform.then(block_transform);
        let current_block = SceneBlock::new(
            node.id().0,
            current_layer_id,
            *next_order,
            world_transform.map_rect(local_bounds),
            block_transform,
            None,
            fragment.clone(),
        );
        seen_blocks.insert(current_block.node_id);
        if let Some(previous) = previous_blocks_by_id.get(&current_block.node_id) {
            push_block_patch(previous, &current_block, upserts, reorders);
        } else {
            upserts.push(current_block.clone());
        }
        *next_order += 1;
    }
    collect_scene_patch_children(
        node,
        measured,
        fragments_by_node,
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

fn collect_scene_patch_children(
    node: &Node,
    measured: &MeasuredNode,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
    current_layer_id: u64,
    current_layer_origin: Point,
    current_layer_world_transform: Transform2D,
    force_update: bool,
    update_ids: &HashSet<NodeId>,
    next_order: &mut u32,
    previous_layers_by_id: &HashMap<u64, &zeno_graphics::SceneLayer>,
    previous_blocks_by_id: &HashMap<u64, &SceneBlock>,
    seen_layers: &mut HashSet<u64>,
    seen_blocks: &mut HashSet<u64>,
    layer_upserts: &mut Vec<zeno_graphics::SceneLayer>,
    layer_reorders: &mut Vec<SceneLayerOrder>,
    upserts: &mut Vec<SceneBlock>,
    reorders: &mut Vec<SceneBlockOrder>,
) {
    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            collect_scene_patch_items(
                child,
                measured_child,
                fragments_by_node,
                current_layer_id,
                current_layer_origin,
                current_layer_world_transform,
                force_update,
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
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            for (child, measured_child) in children.iter().zip(measured_children.iter()) {
                collect_scene_patch_items(
                    child,
                    measured_child,
                    fragments_by_node,
                    current_layer_id,
                    current_layer_origin,
                    current_layer_world_transform,
                    force_update,
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
        _ => {}
    }
}

fn collect_unchanged_scene_items(
    node: &Node,
    measured: &MeasuredNode,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
    current_layer_id: u64,
    current_layer_origin: Point,
    current_layer_world_transform: Transform2D,
    next_order: &mut u32,
    previous_layers_by_id: &HashMap<u64, &zeno_graphics::SceneLayer>,
    previous_blocks_by_id: &HashMap<u64, &SceneBlock>,
    seen_layers: &mut HashSet<u64>,
    seen_blocks: &mut HashSet<u64>,
    layer_reorders: &mut Vec<SceneLayerOrder>,
    reorders: &mut Vec<SceneBlockOrder>,
) {
    let style = node.resolved_style();
    let local_bounds = Rect::new(
        0.0,
        0.0,
        measured.frame.size.width,
        measured.frame.size.height,
    );
    if node_creates_layer(&style) {
        let layer_transform = layer_local_transform(
            measured.frame.origin,
            current_layer_origin,
            measured.frame.size,
            style.transform,
            style.transform_origin,
        );
        let world_transform = current_layer_world_transform.then(layer_transform);
        let effect_bounds = scene_effect_bounds(local_bounds, &style);
        let current_layer = zeno_graphics::SceneLayer::new(
            node.id().0,
            node.id().0,
            Some(current_layer_id),
            *next_order,
            local_bounds,
            world_transform.map_rect(effect_bounds),
            layer_transform,
            scene_clip(measured.frame.size, style.clip),
            style.opacity,
            scene_blend_mode(style.blend_mode),
            scene_effects(&style),
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
        if let Some(fragment) = fragments_by_node.get(&node.id()) {
            let current_block = SceneBlock::new(
                node.id().0,
                current_layer.layer_id,
                *next_order,
                world_transform.map_rect(local_bounds),
                Transform2D::identity(),
                None,
                fragment.clone(),
            );
            seen_blocks.insert(current_block.node_id);
            if let Some(previous) = previous_blocks_by_id.get(&current_block.node_id) {
                push_block_patch(previous, &current_block, &mut Vec::new(), reorders);
            }
            *next_order += 1;
        }
        match (&node.kind, &measured.kind) {
            (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
                collect_unchanged_scene_items(
                    child,
                    measured_child,
                    fragments_by_node,
                    current_layer.layer_id,
                    measured.frame.origin,
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
            (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
                for (child, measured_child) in children.iter().zip(measured_children.iter()) {
                    collect_unchanged_scene_items(
                        child,
                        measured_child,
                        fragments_by_node,
                        current_layer.layer_id,
                        measured.frame.origin,
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
            }
            _ => {}
        }
        return;
    }

    if let Some(fragment) = fragments_by_node.get(&node.id()) {
        let block_transform = Transform2D::translation(
            measured.frame.origin.x - current_layer_origin.x,
            measured.frame.origin.y - current_layer_origin.y,
        );
        let world_transform = current_layer_world_transform.then(block_transform);
        let current_block = SceneBlock::new(
            node.id().0,
            current_layer_id,
            *next_order,
            world_transform.map_rect(local_bounds),
            block_transform,
            None,
            fragment.clone(),
        );
        seen_blocks.insert(current_block.node_id);
        if let Some(previous) = previous_blocks_by_id.get(&current_block.node_id) {
            push_block_patch(previous, &current_block, &mut Vec::new(), reorders);
        }
        *next_order += 1;
    }
    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            collect_unchanged_scene_items(
                child,
                measured_child,
                fragments_by_node,
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
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            for (child, measured_child) in children.iter().zip(measured_children.iter()) {
                collect_unchanged_scene_items(
                    child,
                    measured_child,
                    fragments_by_node,
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
        _ => {}
    }
}
