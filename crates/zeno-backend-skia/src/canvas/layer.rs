use skia_safe as sk;
use zeno_scene::{DrawOp, RenderObject, RetainedScene, Scene};

use crate::canvas::{
    draw::draw_command,
    effects::{layer_effect_bounds, layer_paint, needs_save_layer},
    mapping::{apply_clip, apply_transform},
    text::SkiaTextCache,
};

pub(crate) fn render_retained_scene_layers(
    canvas: &sk::Canvas,
    scene: &mut RetainedScene,
    text_cache: &mut SkiaTextCache,
) {
    let mut save_stack: Vec<Option<usize>> = Vec::new();
    let ops = scene.draw_ops().to_vec();
    for op in ops {
        match op {
            DrawOp::EnterLayer(layer_index) => {
                let layer = scene.layer(layer_index);
                if layer.layer_id == Scene::ROOT_LAYER_ID {
                    save_stack.push(None);
                    continue;
                }
                let initial_save_count = canvas.save_count();
                canvas.save();
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
                save_stack.push(Some(initial_save_count));
            }
            DrawOp::DrawObject(object_index) => {
                let object = scene.object(object_index);
                draw_object(canvas, scene, object, object_index, text_cache);
            }
            DrawOp::ExitLayer(_) => {
                if let Some(Some(save_count)) = save_stack.pop() {
                    canvas.restore_to_count(save_count);
                }
            }
        }
    }
}

fn draw_object(
    canvas: &sk::Canvas,
    scene: &RetainedScene,
    object: &RenderObject,
    object_index: usize,
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
    for cmd in scene.packets_for_object_index(object_index) {
        draw_command(canvas, cmd, text_cache);
    }
    if needs_save {
        canvas.restore();
    }
}
