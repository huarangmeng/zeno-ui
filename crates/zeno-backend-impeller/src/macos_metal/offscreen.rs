use std::collections::HashMap;

use fontdue::Font;
use metal::{
    CommandBufferRef, Device, MTLClearColor, MTLLoadAction, MTLPrimitiveType, MTLScissorRect,
    MTLStoreAction, RenderPassDescriptor, RenderPipelineState, Texture,
};
use zeno_core::{Rect, Transform2D};
use zeno_graphics::{Scene, SceneBlendMode, SceneBlock, SceneEffect, SceneLayer};

use super::{
    draw::{
        CompositeVertex, build_composite_vertices, color_to_f32, make_offscreen_texture, new_buffer,
    },
    layer_renderer::render_layer,
    scissor::{
        intersect_scissor, inverse_map_rect, rect_from_scissor, rect_intersection, scissor_for_rect,
    },
    text::{CachedGlyph, GlyphCacheKey},
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

// 这里集中处理离屏合成，便于后续继续向更细粒度的 offscreen patch 演进。
#[allow(clippy::too_many_arguments)]
pub(super) fn render_offscreen_layer(
    device: &Device,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    command_buffer: &CommandBufferRef,
    parent_encoder: &metal::RenderCommandEncoderRef,
    layer: &SceneLayer,
    combined_transform: Transform2D,
    combined_opacity: f32,
    parent_scissor: MTLScissorRect,
    layers_by_id: &HashMap<u64, &SceneLayer>,
    child_layers_by_parent: &HashMap<u64, Vec<&SceneLayer>>,
    blocks_by_layer: &HashMap<u64, Vec<&SceneBlock>>,
    parent_viewport_width: f32,
    parent_viewport_height: f32,
    glyph_cache: &mut HashMap<GlyphCacheKey, CachedGlyph>,
) {
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
    let offscreen_encoder = command_buffer.new_render_command_encoder(&render_pass);
    let offscreen_width = texture_width as f32;
    let offscreen_height = texture_height as f32;
    let parent_dirty_rect = rect_from_scissor(parent_scissor);
    let local_dirty_rect = inverse_map_rect(combined_transform, parent_dirty_rect)
        .and_then(|bounds| rect_intersection(bounds, effect_bounds))
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
    render_layer(
        device,
        color_pipeline,
        text_pipeline,
        composite_pipeline,
        composite_multiply_pipeline,
        composite_screen_pipeline,
        font,
        command_buffer,
        &offscreen_encoder,
        layer,
        Transform2D::translation(-effect_bounds.origin.x, -effect_bounds.origin.y),
        1.0,
        offscreen_scissor,
        layers_by_id,
        child_layers_by_parent,
        blocks_by_layer,
        offscreen_width,
        offscreen_height,
        glyph_cache,
    );
    offscreen_encoder.end_encoding();

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

pub(super) fn should_render_offscreen(layer: &SceneLayer) -> bool {
    layer.layer_id != Scene::ROOT_LAYER_ID
        && (layer.offscreen
            || layer.blend_mode != SceneBlendMode::Normal
            || !layer.effects.is_empty())
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
    layer: &SceneLayer,
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

pub(super) fn local_effect_bounds(layer: &SceneLayer) -> Rect {
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

pub(super) fn expand_rect(rect: Rect, amount: f32) -> Rect {
    Rect::new(
        rect.origin.x - amount,
        rect.origin.y - amount,
        rect.size.width + amount * 2.0,
        rect.size.height + amount * 2.0,
    )
}
