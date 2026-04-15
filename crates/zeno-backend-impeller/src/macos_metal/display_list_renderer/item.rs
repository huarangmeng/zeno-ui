use fontdue::Font;
use metal::{Device, MTLScissorRect, RenderPipelineState};
use zeno_core::Transform2D;
use zeno_scene::{DisplayItem, DisplayItemPayload, DisplayList};
use zeno_text::GlyphRasterCache;

use super::super::draw::{
    build_composite_vertices, build_shape_vertices, build_text_vertices, make_text_texture,
    new_buffer,
};
use super::super::offscreen::composite_pipeline_for_blend;
use super::super::scissor::intersect_scissor;
use super::super::text::rasterize_layout;
use super::cache::ImageTextureCache;
use super::helpers::{
    apply_alpha, clip_scissor_with_lookups, composite_params_for_effects,
    world_transform_with_lookups,
};
use super::lookups::RenderLookupTables;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_item(
    device: &Device,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    encoder: &metal::RenderCommandEncoderRef,
    display_list: &DisplayList,
    item: &DisplayItem,
    parent_transform: Transform2D,
    opacity: f32,
    parent_scissor: MTLScissorRect,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &GlyphRasterCache,
    render_lookups: &RenderLookupTables,
    image_texture_cache: &mut ImageTextureCache,
) {
    let transform = parent_transform.then(world_transform_with_lookups(
        render_lookups,
        item.spatial_id,
    ));
    let scissor = intersect_scissor(
        parent_scissor,
        clip_scissor_with_lookups(
            display_list,
            render_lookups,
            item.clip_chain_id,
            parent_transform,
            viewport_width,
            viewport_height,
        ),
    );
    encoder.set_scissor_rect(scissor);

    match &item.payload {
        DisplayItemPayload::FillRect { rect, color } => {
            if let Some(vertices) = build_shape_vertices(
                &zeno_scene::Shape::Rect(*rect),
                apply_alpha(*color, opacity),
                viewport_width,
                viewport_height,
                transform,
            ) {
                let buffer = new_buffer(device, &vertices);
                encoder.set_render_pipeline_state(color_pipeline);
                encoder.set_vertex_buffer(0, Some(&buffer), 0);
                encoder.draw_primitives(
                    metal::MTLPrimitiveType::Triangle,
                    0,
                    vertices.len() as u64,
                );
            }
        }
        DisplayItemPayload::FillRoundedRect {
            rect,
            radius,
            color,
        } => {
            if let Some(vertices) = build_shape_vertices(
                &zeno_scene::Shape::RoundedRect {
                    rect: *rect,
                    radius: *radius,
                },
                apply_alpha(*color, opacity),
                viewport_width,
                viewport_height,
                transform,
            ) {
                let buffer = new_buffer(device, &vertices);
                encoder.set_render_pipeline_state(color_pipeline);
                encoder.set_vertex_buffer(0, Some(&buffer), 0);
                encoder.draw_primitives(
                    metal::MTLPrimitiveType::Triangle,
                    0,
                    vertices.len() as u64,
                );
            }
        }
        DisplayItemPayload::TextRun(text) => {
            let Some(font) = font else {
                encoder.set_scissor_rect(parent_scissor);
                return;
            };
            let Some((mask, width, height)) =
                rasterize_layout(&text.layout, |glyph_id, glyph, font_size| {
                    Some(glyph_cache.get_or_rasterize(font, glyph_id, glyph, font_size))
                })
            else {
                encoder.set_scissor_rect(parent_scissor);
                return;
            };
            let texture = make_text_texture(device, &mask, width, height);
            let mapped = transform.map_point(zeno_core::Point::new(
                text.position.x,
                text.position.y - text.layout.metrics.ascent,
            ));
            let vertices = build_text_vertices(
                mapped.x,
                mapped.y,
                width as f32,
                height as f32,
                apply_alpha(text.color, opacity),
                viewport_width,
                viewport_height,
            );
            let buffer = new_buffer(device, &vertices);
            encoder.set_render_pipeline_state(text_pipeline);
            encoder.set_vertex_buffer(0, Some(&buffer), 0);
            encoder.set_fragment_texture(0, Some(&texture));
            encoder.draw_primitives(metal::MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
        }
        DisplayItemPayload::Image(image) => {
            let texture = image_texture_cache.texture_for_image(
                device,
                image.cache_key,
                &image.rgba8,
                image.width,
                image.height,
            );
            let rect = transform.map_rect(image.dest_rect);
            let vertices = build_composite_vertices(rect, opacity, viewport_width, viewport_height);
            let buffer = new_buffer(device, &vertices);
            encoder.set_render_pipeline_state(composite_pipeline_for_blend(
                zeno_scene::SceneBlendMode::Normal,
                composite_pipeline,
                composite_multiply_pipeline,
                composite_screen_pipeline,
            ));
            encoder.set_vertex_buffer(0, Some(&buffer), 0);
            encoder.set_fragment_texture(0, Some(&texture));
            let params = composite_params_for_effects(&[], image.width as f32, image.height as f32);
            let params_buffer = new_buffer(device, &[params]);
            encoder.set_fragment_buffer(0, Some(&params_buffer), 0);
            encoder.draw_primitives(metal::MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
        }
        DisplayItemPayload::Custom => {}
    }
    encoder.set_scissor_rect(parent_scissor);
}
