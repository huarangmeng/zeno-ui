use std::collections::HashMap;

use skia_safe as sk;
use zeno_scene::{LayerObject, RenderObject, Scene};

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
    let layers_by_id: HashMap<u64, &LayerObject> = scene
        .layer_graph
        .iter()
        .map(|layer| (layer.layer_id, layer))
        .collect();
    let mut child_layers_by_parent: HashMap<u64, Vec<&LayerObject>> = HashMap::new();
    let mut objects_by_layer: HashMap<u64, Vec<&RenderObject>> = HashMap::new();

    for layer in &scene.layer_graph {
        if let Some(parent_id) = layer.parent_layer_id {
            child_layers_by_parent
                .entry(parent_id)
                .or_default()
                .push(layer);
        }
    }
    for object in &scene.objects {
        objects_by_layer
            .entry(object.layer_id)
            .or_default()
            .push(object);
    }

    render_layer(
        canvas,
        scene,
        Scene::ROOT_LAYER_ID,
        &layers_by_id,
        &child_layers_by_parent,
        &objects_by_layer,
        text_cache,
    );
}

fn render_layer(
    canvas: &sk::Canvas,
    scene: &Scene,
    layer_id: u64,
    layers_by_id: &HashMap<u64, &LayerObject>,
    child_layers_by_parent: &HashMap<u64, Vec<&LayerObject>>,
    objects_by_layer: &HashMap<u64, Vec<&RenderObject>>,
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
    if let Some(objects) = objects_by_layer.get(&layer_id) {
        for object in objects {
            items.push((object.order, LayerItem::Object(*object)));
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
            LayerItem::Object(object) => draw_object(canvas, scene, object, text_cache),
            LayerItem::Layer(child_layer_id) => render_layer(
                canvas,
                scene,
                child_layer_id,
                layers_by_id,
                child_layers_by_parent,
                &objects_by_layer,
                text_cache,
            ),
        }
    }

    if saved {
        canvas.restore_to_count(initial_save_count);
    }
}

fn draw_object(
    canvas: &sk::Canvas,
    scene: &Scene,
    object: &RenderObject,
    text_cache: &mut SkiaTextCache,
) {
    let needs_save = !object.transform.is_identity() || object.clip.is_some();
    if needs_save {
        canvas.save();
    }
    if !object.transform.is_identity() {
        apply_transform(canvas, object.transform);
    }
    if let Some(clip) = object.clip {
        apply_clip(canvas, clip);
    }
    for cmd in scene.packets_for_object(object) {
        draw_command(canvas, cmd, text_cache);
    }
    if needs_save {
        canvas.restore();
    }
}

enum LayerItem<'a> {
    Object(&'a RenderObject),
    Layer(u64),
}
