use fontdue::Font;
use metal::{
    CommandQueue, Device, MTLLoadAction, MTLScissorRect, MTLStoreAction, MetalDrawableRef,
    RenderPassDescriptor, RenderPipelineState, TextureRef,
};
use std::time::Instant;
use zeno_core::{Color, Rect, Transform2D, ZenoError, ZenoErrorCode, zeno_session_log};
use zeno_scene::{BlendMode, CompositorLayerTree, DisplayList, SceneBlendMode, StackingContextId};
use zeno_text::GlyphRasterCache;

#[path = "display_list_renderer/cache.rs"]
mod cache;
#[path = "display_list_renderer/context.rs"]
mod context;
#[path = "display_list_renderer/helpers.rs"]
mod helpers;
#[path = "display_list_renderer/item.rs"]
mod item;
#[path = "display_list_renderer/lookups.rs"]
mod lookups;
#[path = "display_list_renderer/scope.rs"]
mod scope;

pub(crate) use cache::{ImageTextureCache, OffscreenContextCache};
use helpers::context_bounds_with_lookups;
use item::render_item;
use lookups::RenderLookupTables;
use scope::{ScopeStep, scope_steps_with_lookups};

#[cfg(test)]
use helpers::{clip_scissor, context_bounds};
#[cfg(test)]
use scope::{immediate_child_context, scope_steps};

use super::{
    offscreen::{CompositeParams, composite_pipeline_for_blend, draw_composited_texture},
    scissor::effective_root_scissor,
};

// A native DisplayList renderer for the Impeller Metal backend.
// This drives the existing Metal pipelines directly from DisplayList semantics.

pub struct CompositeTextureTile<'a> {
    pub texture: &'a TextureRef,
    pub rect: Rect,
    pub opacity: f32,
    pub blend_mode: SceneBlendMode,
    pub params: CompositeParams,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_display_list_to_drawable_region_with_load(
    device: &Device,
    queue: &CommandQueue,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    drawable: &MetalDrawableRef,
    display_list: &DisplayList,
    clear_color: Option<Color>,
    preserve_contents: bool,
    dirty_bounds: Option<Rect>,
    glyph_cache: &GlyphRasterCache,
    image_texture_cache: &mut ImageTextureCache,
    offscreen_context_cache: &mut OffscreenContextCache,
) -> Result<(), ZenoError> {
    let dirty_regions = dirty_bounds.into_iter().collect::<Vec<_>>();
    render_display_list_to_drawable_tiles_with_load(
        device,
        queue,
        color_pipeline,
        text_pipeline,
        composite_pipeline,
        composite_multiply_pipeline,
        composite_screen_pipeline,
        font,
        drawable,
        display_list,
        clear_color,
        preserve_contents,
        &dirty_regions,
        glyph_cache,
        image_texture_cache,
        offscreen_context_cache,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_display_list_to_drawable_tiles_with_load(
    device: &Device,
    queue: &CommandQueue,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    drawable: &MetalDrawableRef,
    display_list: &DisplayList,
    clear_color: Option<Color>,
    preserve_contents: bool,
    dirty_regions: &[Rect],
    glyph_cache: &GlyphRasterCache,
    image_texture_cache: &mut ImageTextureCache,
    offscreen_context_cache: &mut OffscreenContextCache,
) -> Result<(), ZenoError> {
    render_display_list_to_texture_tiles_with_load(
        device,
        queue,
        color_pipeline,
        text_pipeline,
        composite_pipeline,
        composite_multiply_pipeline,
        composite_screen_pipeline,
        font,
        drawable.texture(),
        display_list,
        clear_color,
        preserve_contents,
        dirty_regions,
        None,
        Transform2D::identity(),
        display_list.viewport.width.max(1.0),
        display_list.viewport.height.max(1.0),
        glyph_cache,
        image_texture_cache,
        offscreen_context_cache,
    )?;
    let command_buffer = queue.new_command_buffer();
    command_buffer.present_drawable(drawable);
    command_buffer.commit();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_display_list_to_texture_tiles_with_load(
    device: &Device,
    queue: &CommandQueue,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    target_texture: &TextureRef,
    display_list: &DisplayList,
    clear_color: Option<Color>,
    preserve_contents: bool,
    dirty_regions: &[Rect],
    layer_tree: Option<&CompositorLayerTree>,
    root_transform: Transform2D,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &GlyphRasterCache,
    image_texture_cache: &mut ImageTextureCache,
    offscreen_context_cache: &mut OffscreenContextCache,
) -> Result<(), ZenoError> {
    let render_pass = RenderPassDescriptor::new();
    let attachment = render_pass
        .color_attachments()
        .object_at(0)
        .ok_or_else(|| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerRenderPassAttachmentMissing,
                "backend.impeller",
                "render_display_list",
                "missing metal color attachment",
            )
        })?;
    attachment.set_texture(Some(target_texture));
    attachment.set_load_action(if preserve_contents {
        MTLLoadAction::Load
    } else {
        MTLLoadAction::Clear
    });
    attachment.set_store_action(MTLStoreAction::Store);
    if !preserve_contents {
        let clear = clear_color.unwrap_or(Color::TRANSPARENT);
        attachment.set_clear_color(metal::MTLClearColor::new(
            f64::from(clear.red) / 255.0,
            f64::from(clear.green) / 255.0,
            f64::from(clear.blue) / 255.0,
            f64::from(clear.alpha) / 255.0,
        ));
    }

    let command_buffer = queue.new_command_buffer();
    let encoder = command_buffer.new_render_command_encoder(&render_pass);
    let render_lookups = RenderLookupTables::build(display_list);
    let root_regions = if dirty_regions.is_empty() {
        vec![Rect::new(0.0, 0.0, viewport_width, viewport_height)]
    } else {
        dirty_regions.to_vec()
    };

    // Stable perf instrumentation. Keep op names in sync with
    // docs/architecture/performance-debugging.md.
    zeno_session_log!(
        trace,
        op = "impeller_display_list_encoder_root",
        preserve_contents,
        dirty_region_count = root_regions.len(),
        items = display_list.items.len(),
        contexts = display_list.stacking_contexts.len(),
        "impeller display list root encoder"
    );
    for dirty_region in root_regions {
        let root_scissor =
            effective_root_scissor(Some(dirty_region), viewport_width, viewport_height);
        encoder.set_scissor_rect(root_scissor);
        let scene_cull_rect = Rect::new(
            dirty_region.origin.x - root_transform.tx,
            dirty_region.origin.y - root_transform.ty,
            dirty_region.size.width,
            dirty_region.size.height,
        );
        // Render stacking contexts in a single pass by grouping draw ops per context.
        // Root scope is `None` stacking context.
        render_scope(
            device,
            queue,
            color_pipeline,
            text_pipeline,
            composite_pipeline,
            composite_multiply_pipeline,
            composite_screen_pipeline,
            font,
            encoder,
            display_list,
            layer_tree,
            None,
            root_transform,
            1.0,
            Some(scene_cull_rect),
            root_scissor,
            viewport_width,
            viewport_height,
            glyph_cache,
            &render_lookups,
            image_texture_cache,
            offscreen_context_cache,
        );
    }

    encoder.end_encoding();
    command_buffer.commit();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn composite_tile_textures_to_drawable_with_load(
    device: &Device,
    queue: &CommandQueue,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    drawable: &MetalDrawableRef,
    clear_color: Option<Color>,
    preserve_contents: bool,
    tiles: &[CompositeTextureTile<'_>],
    viewport_width: f32,
    viewport_height: f32,
) -> Result<(), ZenoError> {
    let composite_started = Instant::now();
    let render_pass = RenderPassDescriptor::new();
    let attachment = render_pass
        .color_attachments()
        .object_at(0)
        .ok_or_else(|| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerRenderPassAttachmentMissing,
                "backend.impeller",
                "composite_tile_textures",
                "missing metal color attachment",
            )
        })?;
    attachment.set_texture(Some(drawable.texture()));
    attachment.set_load_action(if preserve_contents {
        MTLLoadAction::Load
    } else {
        MTLLoadAction::Clear
    });
    attachment.set_store_action(MTLStoreAction::Store);
    if !preserve_contents {
        let clear = clear_color.unwrap_or(Color::TRANSPARENT);
        attachment.set_clear_color(metal::MTLClearColor::new(
            f64::from(clear.red) / 255.0,
            f64::from(clear.green) / 255.0,
            f64::from(clear.blue) / 255.0,
            f64::from(clear.alpha) / 255.0,
        ));
    }
    let command_buffer_started = Instant::now();
    let command_buffer = queue.new_command_buffer();
    let command_buffer_ms = command_buffer_started.elapsed().as_secs_f64() * 1000.0;
    let encoder_started = Instant::now();
    let encoder = command_buffer.new_render_command_encoder(&render_pass);
    let encoder_ms = encoder_started.elapsed().as_secs_f64() * 1000.0;
    let tile_draw_started = Instant::now();
    let mut tile_vertex_build_ms = 0.0;
    let mut tile_buffer_alloc_ms = 0.0;
    let mut tile_encode_ms = 0.0;
    let mut blend_switch_count = 0usize;
    let mut previous_blend_mode = None;
    for tile in tiles {
        if previous_blend_mode != Some(tile.blend_mode) {
            blend_switch_count += 1;
            previous_blend_mode = Some(tile.blend_mode);
        }
        let pipeline = composite_pipeline_for_blend(
            tile.blend_mode,
            composite_pipeline,
            composite_multiply_pipeline,
            composite_screen_pipeline,
        );
        let draw_stats = draw_composited_texture(
            device,
            pipeline,
            encoder,
            tile.texture,
            tile.rect,
            tile.opacity,
            viewport_width,
            viewport_height,
            tile.params,
        );
        tile_vertex_build_ms += draw_stats.vertex_build_ms;
        tile_buffer_alloc_ms += draw_stats.buffer_alloc_ms;
        tile_encode_ms += draw_stats.encode_ms;
    }
    let tile_draw_ms = tile_draw_started.elapsed().as_secs_f64() * 1000.0;
    let end_encoding_started = Instant::now();
    encoder.end_encoding();
    let end_encoding_ms = end_encoding_started.elapsed().as_secs_f64() * 1000.0;
    let present_commit_started = Instant::now();
    command_buffer.present_drawable(drawable);
    command_buffer.commit();
    let present_commit_ms = present_commit_started.elapsed().as_secs_f64() * 1000.0;
    // Stable perf instrumentation. Keep op names in sync with
    // docs/architecture/performance-debugging.md.
    // #region debug-point impeller-composite-drawable
    zeno_session_log!(
        trace,
        op = "impeller_composite_drawable",
        tile_count = tiles.len(),
        blend_switch_count,
        command_buffer_ms,
        encoder_ms,
        tile_draw_ms,
        tile_vertex_build_ms,
        tile_buffer_alloc_ms,
        tile_encode_ms,
        end_encoding_ms,
        present_commit_ms,
        total_ms = composite_started.elapsed().as_secs_f64() * 1000.0,
        preserve_contents,
        "impeller composite drawable timing"
    );
    // #endregion
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_scope(
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
    parent_context: Option<StackingContextId>,
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
    for step in scope_steps_with_lookups(display_list, render_lookups, layer_tree, parent_context) {
        match step {
            ScopeStep::Direct(item_index) => {
                let item = &display_list.items[item_index];
                if let Some(cull_rect) = scene_cull_rect
                    && !item.visual_rect.intersects(&cull_rect)
                {
                    continue;
                }
                render_item(
                    device,
                    color_pipeline,
                    text_pipeline,
                    composite_pipeline,
                    composite_multiply_pipeline,
                    composite_screen_pipeline,
                    font,
                    encoder,
                    display_list,
                    item,
                    parent_transform,
                    parent_opacity,
                    parent_scissor,
                    viewport_width,
                    viewport_height,
                    glyph_cache,
                    render_lookups,
                    image_texture_cache,
                );
            }
            ScopeStep::ChildContext(child) => {
                if let Some(cull_rect) = scene_cull_rect
                    && !context_bounds_with_lookups(render_lookups, child).intersects(&cull_rect)
                {
                    continue;
                }
                render_context(
                    device,
                    queue,
                    color_pipeline,
                    text_pipeline,
                    composite_pipeline,
                    composite_multiply_pipeline,
                    composite_screen_pipeline,
                    font,
                    encoder,
                    display_list,
                    layer_tree,
                    child,
                    parent_transform,
                    parent_opacity,
                    scene_cull_rect,
                    parent_scissor,
                    viewport_width,
                    viewport_height,
                    glyph_cache,
                    render_lookups,
                    image_texture_cache,
                    offscreen_context_cache,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_context(
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

    let needs_offscreen = context.needs_offscreen
        || context.opacity < 1.0
        || context.blend_mode != BlendMode::Normal
        || !context.effects.is_empty();

    if !needs_offscreen {
        // Render in-place with combined opacity.
        render_scope(
            device,
            queue,
            color_pipeline,
            text_pipeline,
            composite_pipeline,
            composite_multiply_pipeline,
            composite_screen_pipeline,
            font,
            encoder,
            display_list,
            layer_tree,
            Some(context_id),
            parent_transform,
            parent_opacity * context.opacity,
            scene_cull_rect,
            parent_scissor,
            viewport_width,
            viewport_height,
            glyph_cache,
            render_lookups,
            image_texture_cache,
            offscreen_context_cache,
        );
        return;
    }

    // Offscreen: draw subtree into a texture, then composite with blend/effects/opacity.
    context::render_offscreen_context(
        render_scope,
        device,
        queue,
        color_pipeline,
        text_pipeline,
        composite_pipeline,
        composite_multiply_pipeline,
        composite_screen_pipeline,
        font,
        encoder,
        display_list,
        layer_tree,
        context_id,
        parent_transform,
        parent_opacity,
        scene_cull_rect,
        parent_scissor,
        viewport_width,
        viewport_height,
        glyph_cache,
        render_lookups,
        image_texture_cache,
        offscreen_context_cache,
    );
}

#[cfg(test)]
mod tests {
    use super::helpers::composite_params_for_effects;
    use super::{ScopeStep, clip_scissor, context_bounds, immediate_child_context, scope_steps};
    use zeno_core::{Color, Rect, Size, Transform2D};
    use zeno_scene::{
        BlendMode, ClipChain, ClipChainId, ClipChainStore, CompositorPlanner, DamageRegion,
        DisplayImage, DisplayItem, DisplayItemId, DisplayItemPayload, DisplayList, Effect,
        SpatialNode, SpatialNodeId, SpatialTree, StackingContext, StackingContextId, TileCache,
    };

    fn sample_display_list() -> DisplayList {
        DisplayList {
            viewport: Size::new(200.0, 100.0),
            items: vec![
                DisplayItem {
                    item_id: DisplayItemId(1),
                    spatial_id: SpatialNodeId(1),
                    clip_chain_id: ClipChainId(1),
                    stacking_context: None,
                    visual_rect: Rect::new(0.0, 0.0, 20.0, 20.0),
                    payload: DisplayItemPayload::FillRect {
                        rect: Rect::new(0.0, 0.0, 20.0, 20.0),
                        color: Color::WHITE,
                    },
                },
                DisplayItem {
                    item_id: DisplayItemId(2),
                    spatial_id: SpatialNodeId(2),
                    clip_chain_id: ClipChainId(2),
                    stacking_context: Some(StackingContextId(10)),
                    visual_rect: Rect::new(10.0, 10.0, 30.0, 30.0),
                    payload: DisplayItemPayload::FillRect {
                        rect: Rect::new(0.0, 0.0, 30.0, 30.0),
                        color: Color::rgba(10, 20, 30, 255),
                    },
                },
                DisplayItem {
                    item_id: DisplayItemId(3),
                    spatial_id: SpatialNodeId(3),
                    clip_chain_id: ClipChainId(3),
                    stacking_context: Some(StackingContextId(11)),
                    visual_rect: Rect::new(40.0, 12.0, 12.0, 18.0),
                    payload: DisplayItemPayload::FillRect {
                        rect: Rect::new(0.0, 0.0, 12.0, 18.0),
                        color: Color::rgba(50, 60, 70, 255),
                    },
                },
            ],
            spatial_tree: SpatialTree {
                nodes: vec![
                    SpatialNode {
                        id: SpatialNodeId(1),
                        parent: None,
                        local_transform: Transform2D::identity(),
                        world_transform: Transform2D::identity(),
                        dirty: false,
                    },
                    SpatialNode {
                        id: SpatialNodeId(2),
                        parent: Some(SpatialNodeId(1)),
                        local_transform: Transform2D::identity(),
                        world_transform: Transform2D::identity(),
                        dirty: false,
                    },
                    SpatialNode {
                        id: SpatialNodeId(3),
                        parent: Some(SpatialNodeId(2)),
                        local_transform: Transform2D::identity(),
                        world_transform: Transform2D::identity(),
                        dirty: false,
                    },
                ],
            },
            clip_chains: ClipChainStore {
                chains: vec![
                    ClipChain {
                        id: ClipChainId(1),
                        spatial_id: SpatialNodeId(1),
                        clip: zeno_scene::ClipRegion::Rect(Rect::new(0.0, 0.0, 200.0, 100.0)),
                        parent: None,
                    },
                    ClipChain {
                        id: ClipChainId(2),
                        spatial_id: SpatialNodeId(2),
                        clip: zeno_scene::ClipRegion::Rect(Rect::new(10.0, 10.0, 80.0, 60.0)),
                        parent: Some(ClipChainId(1)),
                    },
                    ClipChain {
                        id: ClipChainId(3),
                        spatial_id: SpatialNodeId(3),
                        clip: zeno_scene::ClipRegion::Rect(Rect::new(20.0, 10.0, 40.0, 40.0)),
                        parent: Some(ClipChainId(2)),
                    },
                ],
            },
            stacking_contexts: vec![
                StackingContext {
                    id: StackingContextId(10),
                    parent: None,
                    paint_order: 1,
                    spatial_id: SpatialNodeId(2),
                    opacity: 0.8,
                    blend_mode: BlendMode::Normal,
                    effects: vec![],
                    needs_offscreen: false,
                },
                StackingContext {
                    id: StackingContextId(11),
                    parent: Some(StackingContextId(10)),
                    paint_order: 2,
                    spatial_id: SpatialNodeId(3),
                    opacity: 0.5,
                    blend_mode: BlendMode::Multiply,
                    effects: vec![Effect::Blur { sigma: 2.0 }],
                    needs_offscreen: true,
                },
            ],
            generation: 1,
        }
    }

    #[test]
    fn immediate_child_context_picks_first_descendant_under_scope() {
        let display_list = sample_display_list();

        assert_eq!(
            immediate_child_context(&display_list, None, StackingContextId(11)),
            Some(StackingContextId(10))
        );
        assert_eq!(
            immediate_child_context(
                &display_list,
                Some(StackingContextId(10)),
                StackingContextId(11)
            ),
            Some(StackingContextId(11))
        );
    }

    #[test]
    fn context_bounds_include_descendant_items() {
        let display_list = sample_display_list();

        assert_eq!(
            context_bounds(&display_list, StackingContextId(10)),
            Rect::new(10.0, 10.0, 42.0, 30.0)
        );
        assert_eq!(
            context_bounds(&display_list, StackingContextId(11)),
            Rect::new(40.0, 12.0, 12.0, 18.0)
        );
    }

    #[test]
    fn composite_params_map_display_effects_to_shader_flags() {
        let params = composite_params_for_effects(
            &[
                Effect::Blur { sigma: 3.0 },
                Effect::DropShadow {
                    dx: 4.0,
                    dy: 6.0,
                    blur: 5.0,
                    color: Color::rgba(10, 20, 30, 128),
                },
            ],
            64.0,
            32.0,
        );

        assert_eq!(params.flags, 3);
        assert_eq!(params.blur_sigma, 3.0);
        assert_eq!(params.shadow_blur, 5.0);
        assert_eq!(params.shadow_offset, [4.0, 6.0]);
        assert_eq!(
            params.shadow_color,
            [10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 128.0 / 255.0]
        );
    }

    #[test]
    fn clip_scissor_intersects_parent_chain_bounds() {
        let mut display_list = sample_display_list();
        display_list.clip_chains.chains[1].clip =
            zeno_scene::ClipRegion::Rect(Rect::new(10.0, 10.0, 20.0, 20.0));
        display_list.clip_chains.chains[2].clip =
            zeno_scene::ClipRegion::Rect(Rect::new(0.0, 0.0, 80.0, 80.0));

        let scissor = clip_scissor(
            &display_list,
            ClipChainId(3),
            Transform2D::identity(),
            200.0,
            100.0,
        );

        assert_eq!(scissor.x, 10);
        assert_eq!(scissor.y, 10);
        assert_eq!(scissor.width, 20);
        assert_eq!(scissor.height, 20);
    }

    #[test]
    fn root_scope_keeps_context_at_first_occurrence_without_duplicates() {
        let mut display_list = sample_display_list();
        display_list.items.insert(
            1,
            DisplayItem {
                item_id: DisplayItemId(99),
                spatial_id: SpatialNodeId(1),
                clip_chain_id: ClipChainId(1),
                stacking_context: None,
                visual_rect: Rect::new(5.0, 5.0, 6.0, 6.0),
                payload: DisplayItemPayload::FillRect {
                    rect: Rect::new(5.0, 5.0, 6.0, 6.0),
                    color: Color::rgba(200, 10, 10, 255),
                },
            },
        );
        display_list.items.push(DisplayItem {
            item_id: DisplayItemId(100),
            spatial_id: SpatialNodeId(2),
            clip_chain_id: ClipChainId(2),
            stacking_context: Some(StackingContextId(10)),
            visual_rect: Rect::new(60.0, 10.0, 10.0, 10.0),
            payload: DisplayItemPayload::FillRect {
                rect: Rect::new(0.0, 0.0, 10.0, 10.0),
                color: Color::rgba(20, 20, 200, 255),
            },
        });

        let steps = scope_steps(&display_list, None, None);

        assert_eq!(
            steps,
            vec![
                ScopeStep::Direct(0),
                ScopeStep::Direct(1),
                ScopeStep::ChildContext(StackingContextId(10)),
            ]
        );
    }

    #[test]
    fn nested_scope_keeps_direct_items_before_child_context() {
        let mut display_list = sample_display_list();
        display_list.items.insert(
            2,
            DisplayItem {
                item_id: DisplayItemId(101),
                spatial_id: SpatialNodeId(2),
                clip_chain_id: ClipChainId(2),
                stacking_context: Some(StackingContextId(10)),
                visual_rect: Rect::new(14.0, 14.0, 8.0, 8.0),
                payload: DisplayItemPayload::FillRect {
                    rect: Rect::new(0.0, 0.0, 8.0, 8.0),
                    color: Color::rgba(40, 180, 80, 255),
                },
            },
        );

        let steps = scope_steps(&display_list, None, Some(StackingContextId(10)));

        assert_eq!(
            steps,
            vec![
                ScopeStep::Direct(1),
                ScopeStep::Direct(2),
                ScopeStep::ChildContext(StackingContextId(11)),
            ]
        );
    }

    #[test]
    fn planner_layer_tree_preserves_scope_entry_order_for_backend() {
        let mut display_list = sample_display_list();
        display_list.items.insert(
            1,
            DisplayItem {
                item_id: DisplayItemId(150),
                spatial_id: SpatialNodeId(1),
                clip_chain_id: ClipChainId(1),
                stacking_context: None,
                visual_rect: Rect::new(5.0, 5.0, 6.0, 6.0),
                payload: DisplayItemPayload::FillRect {
                    rect: Rect::new(5.0, 5.0, 6.0, 6.0),
                    color: Color::rgba(200, 10, 10, 255),
                },
            },
        );
        let mut cache = TileCache::new();
        let submission =
            CompositorPlanner::new().plan(&display_list, &mut cache, &DamageRegion::Full);

        assert_eq!(
            scope_steps(&display_list, Some(&submission.layer_tree), None),
            vec![
                ScopeStep::Direct(0),
                ScopeStep::Direct(1),
                ScopeStep::ChildContext(StackingContextId(10)),
            ]
        );
        assert_eq!(
            scope_steps(
                &display_list,
                Some(&submission.layer_tree),
                Some(StackingContextId(10)),
            ),
            vec![
                ScopeStep::Direct(2),
                ScopeStep::ChildContext(StackingContextId(11)),
            ]
        );
    }

    #[test]
    fn image_item_participates_in_root_scope_ordering() {
        let mut display_list = sample_display_list();
        display_list.items.insert(
            1,
            DisplayItem {
                item_id: DisplayItemId(102),
                spatial_id: SpatialNodeId(1),
                clip_chain_id: ClipChainId(1),
                stacking_context: None,
                visual_rect: Rect::new(24.0, 4.0, 16.0, 12.0),
                payload: DisplayItemPayload::Image(DisplayImage::new_rgba8(
                    zeno_scene::ImageCacheKey(7),
                    Rect::new(24.0, 4.0, 16.0, 12.0),
                    2,
                    2,
                    vec![
                        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
                    ],
                )),
            },
        );

        let steps = scope_steps(&display_list, None, None);

        assert_eq!(
            steps,
            vec![
                ScopeStep::Direct(0),
                ScopeStep::Direct(1),
                ScopeStep::ChildContext(StackingContextId(10)),
            ]
        );
    }
}
