use std::time::Instant;

use fontdue::Font;
use metal::{
    CommandQueue, Device, MTLLoadAction, MTLScissorRect, MTLStoreAction, RenderPassDescriptor,
    RenderPipelineState, TextureRef,
};
use zeno_core::{Rect, Transform2D, zeno_session_log};
use zeno_scene::{CompositorLayerTree, DisplayList, StackingContextId};
use zeno_text::GlyphRasterCache;

use super::super::draw::make_offscreen_texture;
use super::super::offscreen::{
    CompositeParams, composite_pipeline_for_blend, draw_composited_texture_region,
};
use super::super::scissor::{intersect_scissor, scissor_for_rect};
use super::cache::{CachedOffscreenContext, ImageTextureCache, OffscreenContextCache};
use super::helpers::{
    apply_effect_bounds, composite_params_for_effects, context_bounds_with_lookups,
    effect_sample_padding, expand_rect, intersect_rect, rect_contains_rect, scene_blend_mode,
    union_patch_rects,
};
use super::lookups::RenderLookupTables;

#[allow(clippy::too_many_arguments)]
pub(super) type RenderScopeFn = fn(
    &Device,
    &CommandQueue,
    &RenderPipelineState,
    &RenderPipelineState,
    &RenderPipelineState,
    &RenderPipelineState,
    &RenderPipelineState,
    Option<&Font>,
    &metal::RenderCommandEncoderRef,
    &DisplayList,
    Option<&CompositorLayerTree>,
    Option<StackingContextId>,
    Transform2D,
    f32,
    Option<Rect>,
    MTLScissorRect,
    f32,
    f32,
    &GlyphRasterCache,
    &RenderLookupTables,
    &mut ImageTextureCache,
    &mut OffscreenContextCache,
);

#[allow(clippy::too_many_arguments)]
pub(super) fn render_offscreen_context(
    render_scope_fn: RenderScopeFn,
    device: &Device,
    queue: &CommandQueue,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    encoder: &metal::RenderCommandEncoderRef,
    display_list: &DisplayList,
    layer_tree: Option<&CompositorLayerTree>,
    context_id: StackingContextId,
    parent_transform: Transform2D,
    parent_opacity: f32,
    scene_cull_rect: Option<Rect>,
    parent_scissor: MTLScissorRect,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &GlyphRasterCache,
    render_lookups: &RenderLookupTables,
    image_texture_cache: &mut ImageTextureCache,
    offscreen_context_cache: &mut OffscreenContextCache,
) {
    let Some(context_index) = render_lookups.context_index(context_id) else {
        return;
    };
    let Some(context) = display_list.stacking_contexts.get(context_index) else {
        return;
    };

    let bounds = context_bounds_with_lookups(render_lookups, context_id);
    let effect_bounds = apply_effect_bounds(bounds, &context.effects);
    let raster_scene_rect = scene_cull_rect
        .map(|cull_rect| expand_rect(cull_rect, effect_sample_padding(&context.effects)))
        .and_then(|expanded_cull| intersect_rect(effect_bounds, expanded_cull))
        .unwrap_or(effect_bounds);
    let visible_scene_rect = scene_cull_rect
        .and_then(|cull_rect| intersect_rect(effect_bounds, cull_rect))
        .unwrap_or(effect_bounds);
    if visible_scene_rect.size.width <= 0.0 || visible_scene_rect.size.height <= 0.0 {
        return;
    }
    let requested_texture_scene_bounds = raster_scene_rect;
    // Effects like blur/drop-shadow need neighborhood samples outside the currently visible tile.
    // Reusing a progressively-grown patch cache and compositing each tile immediately causes the
    // earlier tiles to sample from an incomplete texture, which shows up as tile seams and
    // incorrect bleed. Keep the patch cache for pure offscreen layers, but render effect layers
    // tile-locally for correctness.
    let allow_patch_cache = context.effects.is_empty();
    let cache_hit_entry = if allow_patch_cache {
        offscreen_context_cache.get(context_id)
    } else {
        None
    };
    let requested_texture_width = requested_texture_scene_bounds.size.width.max(1.0).ceil() as u64;
    let requested_texture_height =
        requested_texture_scene_bounds.size.height.max(1.0).ceil() as u64;

    // Stable perf instrumentation. Keep op names in sync with
    // docs/architecture/performance-debugging.md.
    zeno_session_log!(
        trace,
        op = "impeller_offscreen_context",
        ?context_id,
        ?bounds,
        ?effect_bounds,
        ?raster_scene_rect,
        ?visible_scene_rect,
        ?requested_texture_scene_bounds,
        ?scene_cull_rect,
        texture_width = requested_texture_width,
        texture_height = requested_texture_height,
        "impeller offscreen context raster"
    );

    let (
        texture,
        cached_texture_scene_bounds,
        cached_texture_width,
        cached_texture_height,
        texture_alloc_ms,
        texture_grow_copy_ms,
        offscreen_scope_ms,
        cache_hit,
        covered_rect,
    ) = if let Some(entry) = cache_hit_entry.clone() {
        if rect_contains_rect(entry.covered_rect, raster_scene_rect) {
            (
                entry.texture,
                entry.texture_scene_bounds,
                entry.texture_width,
                entry.texture_height,
                0.0,
                0.0,
                0.0,
                true,
                entry.covered_rect,
            )
        } else {
            let patch_scene_rect = entry.covered_rect.union(&raster_scene_rect);
            let (
                texture,
                texture_scene_bounds,
                texture_width,
                texture_height,
                texture_grow_copy_ms,
            ) = if rect_contains_rect(entry.texture_scene_bounds, patch_scene_rect) {
                (
                    entry.texture.clone(),
                    entry.texture_scene_bounds,
                    entry.texture_width,
                    entry.texture_height,
                    0.0,
                )
            } else {
                let grown_texture_scene_bounds =
                    entry.texture_scene_bounds.union(&raster_scene_rect);
                let grown_texture_width =
                    grown_texture_scene_bounds.size.width.max(1.0).ceil() as u64;
                let grown_texture_height =
                    grown_texture_scene_bounds.size.height.max(1.0).ceil() as u64;
                let texture_grow_started = Instant::now();
                let grown_texture = grow_offscreen_texture_with_copy(
                    device,
                    queue,
                    composite_pipeline,
                    &entry.texture,
                    entry.texture_scene_bounds,
                    entry.texture_width as f32,
                    entry.texture_height as f32,
                    grown_texture_scene_bounds,
                    grown_texture_width as f32,
                    grown_texture_height as f32,
                );
                (
                    grown_texture,
                    grown_texture_scene_bounds,
                    grown_texture_width,
                    grown_texture_height,
                    texture_grow_started.elapsed().as_secs_f64() * 1000.0,
                )
            };
            let offscreen_scope_ms = union_patch_rects(entry.covered_rect, raster_scene_rect)
                .into_iter()
                .map(|patch_rect| {
                    rasterize_offscreen_context_patch(
                        render_scope_fn,
                        device,
                        queue,
                        color_pipeline,
                        text_pipeline,
                        composite_pipeline,
                        composite_multiply_pipeline,
                        composite_screen_pipeline,
                        font,
                        &texture,
                        display_list,
                        layer_tree,
                        context_id,
                        texture_scene_bounds,
                        patch_rect,
                        texture_width as f32,
                        texture_height as f32,
                        glyph_cache,
                        render_lookups,
                        image_texture_cache,
                        offscreen_context_cache,
                        true,
                    )
                })
                .sum();
            if allow_patch_cache {
                offscreen_context_cache.insert(
                    context_id,
                    CachedOffscreenContext {
                        texture: texture.clone(),
                        texture_scene_bounds,
                        texture_width,
                        texture_height,
                        covered_rect: patch_scene_rect,
                    },
                );
            }
            (
                texture,
                texture_scene_bounds,
                texture_width,
                texture_height,
                0.0,
                texture_grow_copy_ms,
                offscreen_scope_ms,
                true,
                patch_scene_rect,
            )
        }
    } else {
        let texture_scene_bounds = requested_texture_scene_bounds;
        let texture_width = texture_scene_bounds.size.width.max(1.0).ceil() as u64;
        let texture_height = texture_scene_bounds.size.height.max(1.0).ceil() as u64;
        let texture_alloc_started = Instant::now();
        let texture = make_offscreen_texture(device, texture_width, texture_height);
        let texture_alloc_ms = texture_alloc_started.elapsed().as_secs_f64() * 1000.0;
        let offscreen_scope_ms = rasterize_offscreen_context_patch(
            render_scope_fn,
            device,
            queue,
            color_pipeline,
            text_pipeline,
            composite_pipeline,
            composite_multiply_pipeline,
            composite_screen_pipeline,
            font,
            &texture,
            display_list,
            layer_tree,
            context_id,
            texture_scene_bounds,
            texture_scene_bounds,
            texture_width as f32,
            texture_height as f32,
            glyph_cache,
            render_lookups,
            image_texture_cache,
            offscreen_context_cache,
            false,
        );
        if allow_patch_cache {
            offscreen_context_cache.insert(
                context_id,
                CachedOffscreenContext {
                    texture: texture.clone(),
                    texture_scene_bounds,
                    texture_width,
                    texture_height,
                    covered_rect: texture_scene_bounds,
                },
            );
        }
        (
            texture,
            texture_scene_bounds,
            texture_width,
            texture_height,
            texture_alloc_ms,
            0.0,
            offscreen_scope_ms,
            false,
            texture_scene_bounds,
        )
    };

    // Composite the padded raster rect back into the parent scope, then rely on the parent/tile
    // scissor to clip to the actually visible region. This preserves blur/shadow sampling across
    // tile boundaries instead of reapplying effects on a tightly cropped visible rect.
    let composite_source_scene_rect = raster_scene_rect;
    let composite_rect = parent_transform.map_rect(composite_source_scene_rect);
    let composite_scissor = intersect_scissor(
        parent_scissor,
        scissor_for_rect(composite_rect, viewport_width, viewport_height),
    );
    encoder.set_scissor_rect(composite_scissor);
    let uv_min = [
        ((composite_source_scene_rect.origin.x - cached_texture_scene_bounds.origin.x)
            / cached_texture_scene_bounds.size.width.max(1.0))
        .clamp(0.0, 1.0),
        ((composite_source_scene_rect.origin.y - cached_texture_scene_bounds.origin.y)
            / cached_texture_scene_bounds.size.height.max(1.0))
        .clamp(0.0, 1.0),
    ];
    let uv_max = [
        ((composite_source_scene_rect.right() - cached_texture_scene_bounds.origin.x)
            / cached_texture_scene_bounds.size.width.max(1.0))
        .clamp(0.0, 1.0),
        ((composite_source_scene_rect.bottom() - cached_texture_scene_bounds.origin.y)
            / cached_texture_scene_bounds.size.height.max(1.0))
        .clamp(0.0, 1.0),
    ];
    let composite_back_started = Instant::now();
    let composite_back_stats = draw_composited_texture_region(
        device,
        composite_pipeline_for_blend(
            scene_blend_mode(context.blend_mode),
            composite_pipeline,
            composite_multiply_pipeline,
            composite_screen_pipeline,
        ),
        encoder,
        &texture,
        composite_rect,
        uv_min,
        uv_max,
        parent_opacity * context.opacity,
        viewport_width,
        viewport_height,
        composite_params_for_effects(
            &context.effects,
            cached_texture_width as f32,
            cached_texture_height as f32,
        ),
    );
    let composite_back_ms = composite_back_started.elapsed().as_secs_f64() * 1000.0;
    // Stable perf instrumentation. Keep op names in sync with
    // docs/architecture/performance-debugging.md.
    zeno_session_log!(
        trace,
        op = "impeller_offscreen_context_timing",
        ?context_id,
        texture_alloc_ms,
        texture_grow_copy_ms,
        offscreen_scope_ms,
        composite_back_ms,
        cache_hit,
        ?cached_texture_scene_bounds,
        ?covered_rect,
        composite_back_vertex_build_ms = composite_back_stats.vertex_build_ms,
        composite_back_buffer_alloc_ms = composite_back_stats.buffer_alloc_ms,
        composite_back_encode_ms = composite_back_stats.encode_ms,
        total_ms = texture_alloc_ms + offscreen_scope_ms + composite_back_ms,
        "impeller offscreen context timing"
    );
    encoder.set_scissor_rect(parent_scissor);
}

#[allow(clippy::too_many_arguments)]
fn grow_offscreen_texture_with_copy(
    device: &Device,
    queue: &CommandQueue,
    composite_pipeline: &RenderPipelineState,
    old_texture: &TextureRef,
    old_texture_scene_bounds: Rect,
    old_texture_width: f32,
    old_texture_height: f32,
    new_texture_scene_bounds: Rect,
    new_texture_width: f32,
    new_texture_height: f32,
) -> metal::Texture {
    let new_texture = make_offscreen_texture(
        device,
        new_texture_width.max(1.0).ceil() as u64,
        new_texture_height.max(1.0).ceil() as u64,
    );
    let render_pass = RenderPassDescriptor::new();
    let Some(attachment) = render_pass.color_attachments().object_at(0) else {
        return new_texture;
    };
    attachment.set_texture(Some(&new_texture));
    attachment.set_load_action(MTLLoadAction::Clear);
    attachment.set_store_action(MTLStoreAction::Store);
    attachment.set_clear_color(metal::MTLClearColor::new(0.0, 0.0, 0.0, 0.0));

    let command_buffer = queue.new_command_buffer();
    let encoder = command_buffer.new_render_command_encoder(&render_pass);
    let copy_rect = Rect::new(
        old_texture_scene_bounds.origin.x - new_texture_scene_bounds.origin.x,
        old_texture_scene_bounds.origin.y - new_texture_scene_bounds.origin.y,
        old_texture_scene_bounds.size.width,
        old_texture_scene_bounds.size.height,
    );
    encoder.set_scissor_rect(scissor_for_rect(
        Rect::new(0.0, 0.0, new_texture_width, new_texture_height),
        new_texture_width,
        new_texture_height,
    ));
    let _ = draw_composited_texture_region(
        device,
        composite_pipeline,
        &encoder,
        old_texture,
        copy_rect,
        [0.0, 0.0],
        [1.0, 1.0],
        1.0,
        new_texture_width,
        new_texture_height,
        CompositeParams {
            inv_texture_size: [
                1.0 / old_texture_width.max(1.0),
                1.0 / old_texture_height.max(1.0),
            ],
            ..CompositeParams::default()
        },
    );
    encoder.end_encoding();
    command_buffer.commit();
    new_texture
}

#[allow(clippy::too_many_arguments)]
fn rasterize_offscreen_context_patch(
    render_scope_fn: RenderScopeFn,
    device: &Device,
    queue: &CommandQueue,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    texture: &TextureRef,
    display_list: &DisplayList,
    layer_tree: Option<&CompositorLayerTree>,
    context_id: StackingContextId,
    texture_scene_bounds: Rect,
    patch_scene_rect: Rect,
    texture_width: f32,
    texture_height: f32,
    glyph_cache: &GlyphRasterCache,
    render_lookups: &RenderLookupTables,
    image_texture_cache: &mut ImageTextureCache,
    offscreen_context_cache: &mut OffscreenContextCache,
    preserve_contents: bool,
) -> f64 {
    let render_pass = RenderPassDescriptor::new();
    let Some(attachment) = render_pass.color_attachments().object_at(0) else {
        return 0.0;
    };
    attachment.set_texture(Some(texture));
    attachment.set_load_action(if preserve_contents {
        MTLLoadAction::Load
    } else {
        MTLLoadAction::Clear
    });
    attachment.set_store_action(MTLStoreAction::Store);
    if !preserve_contents {
        attachment.set_clear_color(metal::MTLClearColor::new(0.0, 0.0, 0.0, 0.0));
    }

    let command_buffer = queue.new_command_buffer();
    let encoder = command_buffer.new_render_command_encoder(&render_pass);
    let patch_local_rect = Rect::new(
        patch_scene_rect.origin.x - texture_scene_bounds.origin.x,
        patch_scene_rect.origin.y - texture_scene_bounds.origin.y,
        patch_scene_rect.size.width,
        patch_scene_rect.size.height,
    );
    let off_scissor = scissor_for_rect(patch_local_rect, texture_width, texture_height);
    encoder.set_scissor_rect(off_scissor);
    let offscreen_root = Transform2D::translation(
        -texture_scene_bounds.origin.x,
        -texture_scene_bounds.origin.y,
    );
    let offscreen_scope_started = Instant::now();
    render_scope_fn(
        device,
        queue,
        color_pipeline,
        text_pipeline,
        composite_pipeline,
        composite_multiply_pipeline,
        composite_screen_pipeline,
        font,
        &encoder,
        display_list,
        layer_tree,
        Some(context_id),
        offscreen_root,
        1.0,
        Some(patch_scene_rect),
        off_scissor,
        texture_width,
        texture_height,
        glyph_cache,
        render_lookups,
        image_texture_cache,
        offscreen_context_cache,
    );
    let offscreen_scope_ms = offscreen_scope_started.elapsed().as_secs_f64() * 1000.0;
    encoder.end_encoding();
    command_buffer.commit();
    offscreen_scope_ms
}
