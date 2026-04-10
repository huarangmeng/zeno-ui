use std::collections::HashMap;

use fontdue::Font;
use metal::{CommandQueue, Device, MTLScissorRect, RenderPipelineState};
use zeno_core::Transform2D;
use zeno_scene::{LayerObject, RenderObject, Scene};
use zeno_text::GlyphRasterCache;

use super::{
    draw::draw_commands,
    offscreen::{render_offscreen_layer, should_render_offscreen},
    scissor::{clip_rect, intersect_scissor, scissor_for_rect},
};

enum LayerItem<'a> {
    Object(&'a RenderObject),
    Layer(u64),
}

// 负责按 layer/block 的顺序递归渲染 Scene，主文件只保留渲染入口。
#[allow(clippy::too_many_arguments)]
pub(super) fn render_scene_layers(
    device: &Device,
    queue: &CommandQueue,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    encoder: &metal::RenderCommandEncoderRef,
    scene: &Scene,
    root_scissor: MTLScissorRect,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &GlyphRasterCache,
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
    let Some(root_layer) = layers_by_id.get(&Scene::ROOT_LAYER_ID).copied() else {
        return;
    };
    render_layer(
        device,
        queue,
        color_pipeline,
        text_pipeline,
        composite_pipeline,
        composite_multiply_pipeline,
        composite_screen_pipeline,
        font,
        encoder,
        scene,
        root_layer,
        Transform2D::identity(),
        1.0,
        root_scissor,
        &layers_by_id,
        &child_layers_by_parent,
        &objects_by_layer,
        viewport_width,
        viewport_height,
        glyph_cache,
    );
    encoder.set_scissor_rect(root_scissor);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_layer(
    device: &Device,
    queue: &CommandQueue,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    encoder: &metal::RenderCommandEncoderRef,
    scene: &Scene,
    layer: &LayerObject,
    combined_transform: Transform2D,
    combined_opacity: f32,
    parent_scissor: MTLScissorRect,
    layers_by_id: &HashMap<u64, &LayerObject>,
    child_layers_by_parent: &HashMap<u64, Vec<&LayerObject>>,
    objects_by_layer: &HashMap<u64, Vec<&RenderObject>>,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &GlyphRasterCache,
) {
    let layer_scissor = layer.clip.map_or(parent_scissor, |clip| {
        intersect_scissor(
            parent_scissor,
            scissor_for_rect(
                clip_rect(clip, combined_transform),
                viewport_width,
                viewport_height,
            ),
        )
    });
    encoder.set_scissor_rect(layer_scissor);
    let mut items = Vec::new();
    if let Some(objects) = objects_by_layer.get(&layer.layer_id) {
        for object in objects {
            items.push((object.order, LayerItem::Object(*object)));
        }
    }
    if let Some(children) = child_layers_by_parent.get(&layer.layer_id) {
        for child in children {
            items.push((child.order, LayerItem::Layer(child.layer_id)));
        }
    }
    items.sort_by_key(|(order, _)| *order);
    for (_, item) in items {
        match item {
            LayerItem::Object(object) => {
                let object_transform = combined_transform.then(object.transform);
                let object_scissor = object.clip.map_or(layer_scissor, |clip| {
                    intersect_scissor(
                        layer_scissor,
                        scissor_for_rect(
                            clip_rect(clip, object_transform),
                            viewport_width,
                            viewport_height,
                        ),
                    )
                });
                encoder.set_scissor_rect(object_scissor);
                draw_commands(
                    device,
                    color_pipeline,
                    text_pipeline,
                    font,
                    encoder,
                    scene.packets_for_object(object),
                    viewport_width,
                    viewport_height,
                    object_transform,
                    combined_opacity,
                    glyph_cache,
                );
                encoder.set_scissor_rect(layer_scissor);
            }
            LayerItem::Layer(child_layer_id) => {
                let Some(child_layer) = layers_by_id.get(&child_layer_id).copied() else {
                    continue;
                };
                let child_transform = combined_transform.then(child_layer.transform);
                let child_opacity = combined_opacity * child_layer.opacity;
                if should_render_offscreen(child_layer) {
                    render_offscreen_layer(
                        device,
                        queue,
                        color_pipeline,
                        text_pipeline,
                        composite_pipeline,
                        composite_multiply_pipeline,
                        composite_screen_pipeline,
                        font,
                        encoder,
                        scene,
                        child_layer,
                        child_transform,
                        child_opacity,
                        layer_scissor,
                        layers_by_id,
                        child_layers_by_parent,
                        objects_by_layer,
                        viewport_width,
                        viewport_height,
                        glyph_cache,
                    );
                    encoder.set_scissor_rect(layer_scissor);
                } else {
                    render_layer(
                        device,
                        queue,
                        color_pipeline,
                        text_pipeline,
                        composite_pipeline,
                        composite_multiply_pipeline,
                        composite_screen_pipeline,
                        font,
                        encoder,
                        scene,
                        child_layer,
                        child_transform,
                        child_opacity,
                        layer_scissor,
                        layers_by_id,
                        child_layers_by_parent,
                        objects_by_layer,
                        viewport_width,
                        viewport_height,
                        glyph_cache,
                    );
                }
            }
        }
    }
}
