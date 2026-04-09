use std::collections::HashMap;

use skia_safe as sk;
use zeno_graphics::{Scene, SceneBlock, SceneLayer};

use crate::canvas::{
    draw::draw_command,
    effects::{layer_effect_bounds, layer_paint, needs_save_layer},
    mapping::{apply_clip, apply_transform},
    text::SkiaTextCache,
};

pub(crate) fn render_scene_layers(
    canvas: &sk::Canvas,
    scene: &Scene,
    text_cache: &mut SkiaTextCache,
) {
    let layers_by_id: HashMap<u64, &SceneLayer> = scene
        .layers
        .iter()
        .map(|layer| (layer.layer_id, layer))
        .collect();
    let mut child_layers_by_parent: HashMap<u64, Vec<&SceneLayer>> = HashMap::new();
    let mut blocks_by_layer: HashMap<u64, Vec<&SceneBlock>> = HashMap::new();

    for layer in &scene.layers {
        if let Some(parent_id) = layer.parent_layer_id {
            child_layers_by_parent
                .entry(parent_id)
                .or_default()
                .push(layer);
        }
    }
    for block in &scene.blocks {
        blocks_by_layer
            .entry(block.layer_id)
            .or_default()
            .push(block);
    }

    render_layer(
        canvas,
        Scene::ROOT_LAYER_ID,
        &layers_by_id,
        &child_layers_by_parent,
        &blocks_by_layer,
        text_cache,
    );
}

fn render_layer(
    canvas: &sk::Canvas,
    layer_id: u64,
    layers_by_id: &HashMap<u64, &SceneLayer>,
    child_layers_by_parent: &HashMap<u64, Vec<&SceneLayer>>,
    blocks_by_layer: &HashMap<u64, Vec<&SceneBlock>>,
    text_cache: &mut SkiaTextCache,
) {
    let Some(layer) = layers_by_id.get(&layer_id).copied() else {
        return;
    };

    let initial_save_count = canvas.save_count();
    let mut saved = false;
    if layer.layer_id != Scene::ROOT_LAYER_ID {
        canvas.save();
        saved = true;
        if !layer.transform.is_identity() {
            apply_transform(canvas, layer.transform);
        }
        if let Some(clip) = layer.clip {
            apply_clip(canvas, clip);
        }
        if needs_save_layer(layer) {
            let bounds = layer_effect_bounds(layer);
            let paint = layer_paint(layer);
            let layer_rec = sk::canvas::SaveLayerRec::default()
                .bounds(&bounds)
                .paint(&paint);
            canvas.save_layer(&layer_rec);
        }
    }

    let mut items = Vec::new();
    if let Some(blocks) = blocks_by_layer.get(&layer_id) {
        for block in blocks {
            items.push((block.order, LayerItem::Block(*block)));
        }
    }
    if let Some(children) = child_layers_by_parent.get(&layer_id) {
        for child in children {
            items.push((child.order, LayerItem::Layer(child.layer_id)));
        }
    }

    // block 和子 layer 共用同一套 order 排序，才能和 retained scene 的提交顺序严格一致。
    items.sort_by_key(|(order, _)| *order);
    for (_, item) in items {
        match item {
            LayerItem::Block(block) => draw_block(canvas, block, text_cache),
            LayerItem::Layer(child_layer_id) => render_layer(
                canvas,
                child_layer_id,
                layers_by_id,
                child_layers_by_parent,
                blocks_by_layer,
                text_cache,
            ),
        }
    }

    if saved {
        canvas.restore_to_count(initial_save_count);
    }
}

fn draw_block(canvas: &sk::Canvas, block: &SceneBlock, text_cache: &mut SkiaTextCache) {
    let needs_save = !block.transform.is_identity() || block.clip.is_some();
    if needs_save {
        canvas.save();
    }
    if !block.transform.is_identity() {
        apply_transform(canvas, block.transform);
    }
    if let Some(clip) = block.clip {
        apply_clip(canvas, clip);
    }
    for cmd in &block.commands {
        draw_command(canvas, cmd, text_cache);
    }
    if needs_save {
        canvas.restore();
    }
}

enum LayerItem<'a> {
    Block(&'a SceneBlock),
    Layer(u64),
}
