use fontdue::Font;
use metal::{
    CommandQueue, Device, MTLClearColor, MTLLoadAction, MTLPrimitiveType, MTLScissorRect,
    MTLStoreAction, RenderPassDescriptor, RenderPipelineState, Texture,
};
use zeno_core::{Rect, Transform2D, zeno_session_log};
use zeno_scene::{DrawOp, LayerObject, RetainedScene, Scene, SceneBlendMode, SceneEffect};
use zeno_text::GlyphRasterCache;

use super::{
    draw::{
        CompositeVertex, build_composite_vertices, color_to_f32, draw_commands, make_offscreen_texture, new_buffer,
    },
    scissor::{
        intersect_scissor, inverse_map_rect, rect_from_scissor, rect_intersection, scissor_for_rect,
    },
};

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub(super) struct CompositeParams {
    pub inv_texture_size: [f32; 2],
    pub blur_sigma: f32,
    pub shadow_blur: f32,
    pub shadow_offset: [f32; 2],
    pub shadow_color: [f32; 4],
    pub flags: u32,
    pub _padding: [u32; 3],
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_offscreen_layer_retained(
    device: &Device,
    queue: &CommandQueue,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    parent_encoder: &metal::RenderCommandEncoderRef,
    scene: &mut RetainedScene,
    ops: &[DrawOp],
    enter_index: usize,
    layer_index: usize,
    combined_transform: Transform2D,
    combined_opacity: f32,
    parent_scissor: MTLScissorRect,
    parent_viewport_width: f32,
    parent_viewport_height: f32,
    glyph_cache: &GlyphRasterCache,
) {
    let layer = scene.layer(layer_index);
    let effect_bounds = local_effect_bounds(layer);
    let texture_width = effect_bounds.size.width.max(1.0).ceil() as u64;
    let texture_height = effect_bounds.size.height.max(1.0).ceil() as u64;
    let texture = make_offscreen_texture(device, texture_width, texture_height);
    let render_pass = RenderPassDescriptor::new();
    let Some(attachment) = render_pass.color_attachments().object_at(0) else {
        return;
    };
    attachment.set_texture(Some(&texture));
    attachment.set_load_action(MTLLoadAction::Clear);
    attachment.set_store_action(MTLStoreAction::Store);
    attachment.set_clear_color(MTLClearColor::new(0.0, 0.0, 0.0, 0.0));
    zeno_session_log!(
        trace,
        op = "impeller_encoder_offscreen_begin",
        layer_id = layer.layer_id,
        texture_width,
        texture_height,
        "impeller retained offscreen encoder begin"
    );
    let offscreen_command_buffer = queue.new_command_buffer();
    let offscreen_encoder = offscreen_command_buffer.new_render_command_encoder(&render_pass);
    let offscreen_width = texture_width as f32;
    let offscreen_height = texture_height as f32;
    let parent_dirty_rect = rect_from_scissor(parent_scissor);
    let local_dirty_rect = inverse_map_rect(combined_transform, parent_dirty_rect)
        .and_then(|bounds| rect_intersection(bounds, effect_bounds))
        .map(|bounds| expand_and_clip_rect(bounds, offscreen_sampling_padding(layer), effect_bounds))
        .unwrap_or(effect_bounds);
    let offscreen_scissor = scissor_for_rect(
        Rect::new(
            (local_dirty_rect.origin.x - effect_bounds.origin.x).max(0.0),
            (local_dirty_rect.origin.y - effect_bounds.origin.y).max(0.0),
            local_dirty_rect.size.width,
            local_dirty_rect.size.height,
        ),
        offscreen_width,
        offscreen_height,
    );
    offscreen_encoder.set_scissor_rect(offscreen_scissor);

    render_retained_subtree_into_offscreen(
        device,
        color_pipeline,
        text_pipeline,
        font,
        &offscreen_encoder,
        scene,
        ops,
        enter_index,
        offscreen_scissor,
        offscreen_width,
        offscreen_height,
        glyph_cache,
        effect_bounds.origin.x,
        effect_bounds.origin.y,
    );

    zeno_session_log!(
        trace,
        op = "impeller_encoder_offscreen_end",
        layer_id = layer.layer_id,
        "impeller retained offscreen encoder end"
    );
    offscreen_encoder.end_encoding();
    offscreen_command_buffer.commit();
    offscreen_command_buffer.wait_until_completed();

    let composite_rect = combined_transform.map_rect(effect_bounds);
    let composite_scissor = intersect_scissor(
        parent_scissor,
        scissor_for_rect(
            composite_rect,
            parent_viewport_width,
            parent_viewport_height,
        ),
    );
    parent_encoder.set_scissor_rect(composite_scissor);
    draw_composited_texture(
        device,
        composite_pipeline_for_blend(
            layer.blend_mode,
            composite_pipeline,
            composite_multiply_pipeline,
            composite_screen_pipeline,
        ),
        parent_encoder,
        &texture,
        composite_rect,
        combined_opacity,
        parent_viewport_width,
        parent_viewport_height,
        composite_params(layer, texture_width as f32, texture_height as f32),
    );
}

// 这里集中处理离屏合成，便于后续继续向更细粒度的 offscreen patch 演进。
pub(super) fn should_render_offscreen(layer: &LayerObject) -> bool {
    layer.layer_id != Scene::ROOT_LAYER_ID
        && (layer.offscreen
            || layer.blend_mode != SceneBlendMode::Normal
            || !layer.effects.is_empty())
}

#[allow(clippy::too_many_arguments)]
fn render_retained_subtree_into_offscreen(
    device: &Device,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    encoder: &metal::RenderCommandEncoderRef,
    scene: &RetainedScene,
    ops: &[DrawOp],
    enter_index: usize,
    root_scissor: MTLScissorRect,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &GlyphRasterCache,
    translate_x: f32,
    translate_y: f32,
) {
    let mut stack: Vec<(Transform2D, f32, MTLScissorRect)> = Vec::new();
    let mut depth = 0i32;
    let mut i = enter_index;
    while i < ops.len() {
        match ops[i] {
            DrawOp::EnterLayer(layer_index) => {
                depth += 1;
                let layer = scene.layer(layer_index);
                let (parent_transform, parent_opacity, parent_scissor) = stack
                    .last()
                    .copied()
                    .unwrap_or((
                        Transform2D::translation(-translate_x, -translate_y),
                        1.0,
                        root_scissor,
                    ));
                let combined_transform = if depth == 1 {
                    parent_transform
                } else {
                    parent_transform.then(layer.transform)
                };
                let scissor = layer.clip.map_or(parent_scissor, |clip| {
                    intersect_scissor(
                        parent_scissor,
                        scissor_for_rect(
                            super::scissor::clip_rect(clip, combined_transform),
                            viewport_width,
                            viewport_height,
                        ),
                    )
                });
                encoder.set_scissor_rect(scissor);
                stack.push((combined_transform, parent_opacity * layer.opacity, scissor));
            }
            DrawOp::DrawObject(object_index) => {
                let Some((layer_transform, opacity, layer_scissor)) = stack.last().copied() else {
                    i += 1;
                    continue;
                };
                let object = scene.object(object_index);
                let object_transform = layer_transform.then(object.transform);
                let object_scissor = object.clip.map_or(layer_scissor, |clip| {
                    intersect_scissor(
                        layer_scissor,
                        scissor_for_rect(
                            super::scissor::clip_rect(clip, object_transform),
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
                    scene.packets_for_object_index(object_index),
                    viewport_width,
                    viewport_height,
                    object_transform,
                    opacity,
                    glyph_cache,
                );
                encoder.set_scissor_rect(layer_scissor);
            }
            DrawOp::ExitLayer(_) => {
                depth -= 1;
                let _ = stack.pop();
                let scissor = stack.last().map(|(_, _, s)| *s).unwrap_or(root_scissor);
                encoder.set_scissor_rect(scissor);
                if depth == 0 {
                    break;
                }
            }
        }
        i += 1;
    }
}

pub(super) fn draw_composited_texture(
    device: &Device,
    composite_pipeline: &RenderPipelineState,
    encoder: &metal::RenderCommandEncoderRef,
    texture: &Texture,
    rect: Rect,
    opacity: f32,
    viewport_width: f32,
    viewport_height: f32,
    params: CompositeParams,
) {
    let vertices: Vec<CompositeVertex> =
        build_composite_vertices(rect, opacity, viewport_width, viewport_height);
    let buffer = new_buffer(device, &vertices);
    encoder.set_render_pipeline_state(composite_pipeline);
    encoder.set_vertex_buffer(0, Some(&buffer), 0);
    encoder.set_fragment_texture(0, Some(texture));
    encoder.set_fragment_bytes(
        0,
        std::mem::size_of::<CompositeParams>() as u64,
        (&params as *const CompositeParams).cast(),
    );
    encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
}

pub(super) fn composite_pipeline_for_blend<'a>(
    blend_mode: SceneBlendMode,
    normal: &'a RenderPipelineState,
    multiply: &'a RenderPipelineState,
    screen: &'a RenderPipelineState,
) -> &'a RenderPipelineState {
    match blend_mode {
        SceneBlendMode::Normal => normal,
        SceneBlendMode::Multiply => multiply,
        SceneBlendMode::Screen => screen,
    }
}

pub(super) fn composite_params(
    layer: &LayerObject,
    texture_width: f32,
    texture_height: f32,
) -> CompositeParams {
    let mut blur_sigma = 0.0;
    let mut shadow_blur = 0.0;
    let mut shadow_offset = [0.0, 0.0];
    let mut shadow_color = [0.0, 0.0, 0.0, 0.0];
    let mut flags = 0u32;
    for effect in &layer.effects {
        match effect {
            SceneEffect::Blur { sigma } => {
                blur_sigma = *sigma;
                flags |= 1;
            }
            SceneEffect::DropShadow {
                dx,
                dy,
                blur,
                color,
            } => {
                shadow_blur = *blur;
                shadow_offset = [*dx, *dy];
                shadow_color = color_to_f32(*color);
                flags |= 2;
            }
        }
    }
    CompositeParams {
        inv_texture_size: [1.0 / texture_width.max(1.0), 1.0 / texture_height.max(1.0)],
        blur_sigma,
        shadow_blur,
        shadow_offset,
        shadow_color,
        flags,
        _padding: [0, 0, 0],
    }
}

pub(super) fn local_effect_bounds(layer: &LayerObject) -> Rect {
    let mut bounds = layer.local_bounds;
    for effect in &layer.effects {
        match effect {
            SceneEffect::Blur { sigma } => {
                bounds = expand_rect(bounds, sigma * 3.0);
            }
            SceneEffect::DropShadow { dx, dy, blur, .. } => {
                let shadow_bounds = expand_rect(
                    Rect::new(
                        bounds.origin.x + dx,
                        bounds.origin.y + dy,
                        bounds.size.width,
                        bounds.size.height,
                    ),
                    blur * 3.0,
                );
                bounds = bounds.union(&shadow_bounds);
            }
        }
    }
    bounds
}

pub(super) fn offscreen_sampling_padding(layer: &LayerObject) -> f32 {
    layer
        .effects
        .iter()
        .fold(0.0, |padding, effect| match effect {
            SceneEffect::Blur { sigma } => padding.max(sigma * 3.0),
            SceneEffect::DropShadow { dx, dy, blur, .. } => {
                padding.max(blur * 3.0 + dx.abs().max(dy.abs()))
            }
        })
}

pub(super) fn expand_rect(rect: Rect, amount: f32) -> Rect {
    Rect::new(
        rect.origin.x - amount,
        rect.origin.y - amount,
        rect.size.width + amount * 2.0,
        rect.size.height + amount * 2.0,
    )
}

fn expand_and_clip_rect(rect: Rect, amount: f32, clip: Rect) -> Rect {
    rect_intersection(expand_rect(rect, amount), clip).unwrap_or(clip)
}
