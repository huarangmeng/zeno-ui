use std::time::Instant;

use metal::{Device, MTLPrimitiveType, RenderPipelineState, TextureRef};
use zeno_core::Rect;
use zeno_scene::SceneBlendMode;
#[cfg(test)]
use zeno_scene::{LayerObject, Scene, SceneEffect};

#[cfg(test)]
use super::draw::color_to_f32;
use super::draw::{CompositeVertex, build_composite_vertices_with_uv, new_buffer};

#[repr(C, align(16))]
#[derive(Debug, Default, Clone, Copy)]
pub struct CompositeParams {
    pub inv_texture_size: [f32; 2],
    pub blur_sigma: f32,
    pub shadow_blur: f32,
    pub shadow_offset: [f32; 2],
    pub shadow_color: [f32; 4],
    pub flags: u32,
    pub _padding: [u32; 3],
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct CompositeDrawStats {
    pub vertex_build_ms: f64,
    pub buffer_alloc_ms: f64,
    pub encode_ms: f64,
}

// 这里集中处理离屏合成，便于后续继续向更细粒度的 offscreen patch 演进。
#[cfg(test)]
pub(super) fn should_render_offscreen(layer: &LayerObject) -> bool {
    layer.layer_id != Scene::ROOT_LAYER_ID
        && (layer.offscreen
            || layer.blend_mode != SceneBlendMode::Normal
            || !layer.effects.is_empty())
}

pub(super) fn draw_composited_texture(
    device: &Device,
    composite_pipeline: &RenderPipelineState,
    encoder: &metal::RenderCommandEncoderRef,
    texture: &TextureRef,
    rect: Rect,
    opacity: f32,
    viewport_width: f32,
    viewport_height: f32,
    params: CompositeParams,
) -> CompositeDrawStats {
    draw_composited_texture_region(
        device,
        composite_pipeline,
        encoder,
        texture,
        rect,
        [0.0, 0.0],
        [1.0, 1.0],
        opacity,
        viewport_width,
        viewport_height,
        params,
    )
}

pub(super) fn draw_composited_texture_region(
    device: &Device,
    composite_pipeline: &RenderPipelineState,
    encoder: &metal::RenderCommandEncoderRef,
    texture: &TextureRef,
    rect: Rect,
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    opacity: f32,
    viewport_width: f32,
    viewport_height: f32,
    params: CompositeParams,
) -> CompositeDrawStats {
    let vertex_build_started = Instant::now();
    let vertices: Vec<CompositeVertex> = build_composite_vertices_with_uv(
        rect,
        uv_min,
        uv_max,
        opacity,
        viewport_width,
        viewport_height,
    );
    let vertex_build_ms = vertex_build_started.elapsed().as_secs_f64() * 1000.0;
    let buffer_alloc_started = Instant::now();
    let buffer = new_buffer(device, &vertices);
    let buffer_alloc_ms = buffer_alloc_started.elapsed().as_secs_f64() * 1000.0;
    let encode_started = Instant::now();
    encoder.set_render_pipeline_state(composite_pipeline);
    encoder.set_vertex_buffer(0, Some(&buffer), 0);
    encoder.set_fragment_texture(0, Some(texture));
    encoder.set_fragment_bytes(
        0,
        std::mem::size_of::<CompositeParams>() as u64,
        (&params as *const CompositeParams).cast(),
    );
    encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
    CompositeDrawStats {
        vertex_build_ms,
        buffer_alloc_ms,
        encode_ms: encode_started.elapsed().as_secs_f64() * 1000.0,
    }
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
pub(super) fn expand_rect(rect: Rect, amount: f32) -> Rect {
    Rect::new(
        rect.origin.x - amount,
        rect.origin.y - amount,
        rect.size.width + amount * 2.0,
        rect.size.height + amount * 2.0,
    )
}
