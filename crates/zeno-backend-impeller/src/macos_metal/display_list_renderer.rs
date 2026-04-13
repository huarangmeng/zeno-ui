use fontdue::Font;
use metal::{
    CommandQueue, Device, MTLLoadAction, MTLScissorRect, MTLStoreAction, MetalDrawableRef,
    RenderPassDescriptor, RenderPipelineState,
};
use zeno_core::{Color, Rect, Transform2D, ZenoError, ZenoErrorCode, zeno_session_log};
use zeno_scene::{BlendMode, ClipRegion, DisplayItemPayload, DisplayList, Effect, StackingContextId};
use zeno_text::GlyphRasterCache;

use super::{
    draw::{
        build_composite_vertices, build_shape_vertices, build_text_vertices, make_offscreen_texture,
        make_rgba_texture, make_text_texture, new_buffer,
    },
    offscreen::{CompositeParams, composite_pipeline_for_blend, draw_composited_texture},
    scissor::{effective_root_scissor, intersect_scissor, scissor_for_rect},
    text::rasterize_layout,
};

// A native DisplayList renderer for the Impeller Metal backend.
// This bypasses legacy Scene/RetainedScene and drives the existing pipelines directly.

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

    let command_buffer = queue.new_command_buffer();
    let encoder = command_buffer.new_render_command_encoder(&render_pass);
    let viewport_width = display_list.viewport.width.max(1.0);
    let viewport_height = display_list.viewport.height.max(1.0);
    let root_scissor = effective_root_scissor(dirty_bounds, viewport_width, viewport_height);
    encoder.set_scissor_rect(root_scissor);

    zeno_session_log!(
        trace,
        op = "impeller_display_list_encoder_root",
        preserve_contents,
        ?dirty_bounds,
        items = display_list.items.len(),
        contexts = display_list.stacking_contexts.len(),
        "impeller display list root encoder"
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
        None,
        Transform2D::identity(),
        1.0,
        root_scissor,
        viewport_width,
        viewport_height,
        glyph_cache,
    );

    encoder.end_encoding();
    command_buffer.present_drawable(drawable);
    command_buffer.commit();
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
    parent_context: Option<StackingContextId>,
    parent_transform: Transform2D,
    parent_opacity: f32,
    parent_scissor: MTLScissorRect,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &GlyphRasterCache,
) {
    for step in scope_steps(display_list, parent_context) {
        match step {
            ScopeStep::Direct(item_index) => {
                let item = &display_list.items[item_index];
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
                );
            }
            ScopeStep::ChildContext(child) => {
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
                    child,
                    parent_transform,
                    parent_opacity,
                    parent_scissor,
                    viewport_width,
                    viewport_height,
                    glyph_cache,
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
    context_id: StackingContextId,
    parent_transform: Transform2D,
    parent_opacity: f32,
    parent_scissor: MTLScissorRect,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &GlyphRasterCache,
) {
    let Some(context) = display_list
        .stacking_contexts
        .iter()
        .find(|context| context.id == context_id)
    else {
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
            Some(context_id),
            parent_transform,
            parent_opacity * context.opacity,
            parent_scissor,
            viewport_width,
            viewport_height,
            glyph_cache,
        );
        return;
    }

    // Offscreen: draw subtree into a texture, then composite with blend/effects/opacity.
    let bounds = context_bounds(display_list, context_id);
    let effect_bounds = apply_effect_bounds(bounds, &context.effects);
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
    attachment.set_clear_color(metal::MTLClearColor::new(0.0, 0.0, 0.0, 0.0));

    let offscreen_command_buffer = queue.new_command_buffer();
    let offscreen_encoder = offscreen_command_buffer.new_render_command_encoder(&render_pass);
    let off_w = texture_width as f32;
    let off_h = texture_height as f32;
    let off_scissor = scissor_for_rect(
        Rect::new(0.0, 0.0, off_w, off_h),
        off_w,
        off_h,
    );
    offscreen_encoder.set_scissor_rect(off_scissor);

    // Translate so that offscreen local origin maps to effect_bounds.origin.
    let offscreen_root = Transform2D::translation(-effect_bounds.origin.x, -effect_bounds.origin.y);
    render_scope(
        device,
        queue,
        color_pipeline,
        text_pipeline,
        composite_pipeline,
        composite_multiply_pipeline,
        composite_screen_pipeline,
        font,
        &offscreen_encoder,
        display_list,
        Some(context_id),
        offscreen_root,
        1.0,
        off_scissor,
        off_w,
        off_h,
        glyph_cache,
    );
    offscreen_encoder.end_encoding();
    offscreen_command_buffer.commit();
    offscreen_command_buffer.wait_until_completed();

    // Composite back into parent encoder with context blend/effects.
    let composite_rect = parent_transform.map_rect(effect_bounds);
    let composite_scissor = intersect_scissor(
        parent_scissor,
        scissor_for_rect(composite_rect, viewport_width, viewport_height),
    );
    encoder.set_scissor_rect(composite_scissor);
    draw_composited_texture(
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
        parent_opacity * context.opacity,
        viewport_width,
        viewport_height,
        composite_params_for_effects(&context.effects, texture_width as f32, texture_height as f32),
    );
    encoder.set_scissor_rect(parent_scissor);
}

#[allow(clippy::too_many_arguments)]
fn render_item(
    device: &Device,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    encoder: &metal::RenderCommandEncoderRef,
    display_list: &DisplayList,
    item: &zeno_scene::DisplayItem,
    parent_transform: Transform2D,
    opacity: f32,
    parent_scissor: MTLScissorRect,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &GlyphRasterCache,
) {
    let transform = parent_transform.then(world_transform(display_list, item.spatial_id));
    let scissor = intersect_scissor(
        parent_scissor,
        clip_scissor(
            display_list,
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
                encoder.draw_primitives(metal::MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
            }
        }
        DisplayItemPayload::FillRoundedRect { rect, radius, color } => {
            if let Some(vertices) = build_shape_vertices(
                &zeno_scene::Shape::RoundedRect { rect: *rect, radius: *radius },
                apply_alpha(*color, opacity),
                viewport_width,
                viewport_height,
                transform,
            ) {
                let buffer = new_buffer(device, &vertices);
                encoder.set_render_pipeline_state(color_pipeline);
                encoder.set_vertex_buffer(0, Some(&buffer), 0);
                encoder.draw_primitives(metal::MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
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
            let texture = make_rgba_texture(device, &image.rgba8, image.width, image.height);
            let rect = transform.map_rect(image.dest_rect);
            let vertices = build_composite_vertices(
                rect,
                opacity,
                viewport_width,
                viewport_height,
            );
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

fn apply_alpha(color: Color, opacity: f32) -> Color {
    let mut color = color;
    color.alpha = ((color.alpha as f32) * opacity.clamp(0.0, 1.0)).round() as u8;
    color
}

fn clip_scissor(
    display_list: &DisplayList,
    clip_chain_id: zeno_scene::ClipChainId,
    parent_transform: Transform2D,
    viewport_width: f32,
    viewport_height: f32,
) -> MTLScissorRect {
    let mut scissor = scissor_for_rect(
        Rect::new(0.0, 0.0, viewport_width, viewport_height),
        viewport_width,
        viewport_height,
    );
    let mut current = display_list
        .clip_chains
        .chains
        .iter()
        .find(|chain| chain.id == clip_chain_id);
    while let Some(chain) = current {
        let rect = match &chain.clip {
            ClipRegion::Rect(rect) => *rect,
            ClipRegion::RoundedRect { rect, .. } => *rect,
        };
        let transform = parent_transform.then(world_transform(display_list, chain.spatial_id));
        scissor = intersect_scissor(
            scissor,
            scissor_for_rect(transform.map_rect(rect), viewport_width, viewport_height),
        );
        current = chain.parent.and_then(|parent_id| {
            display_list
                .clip_chains
                .chains
                .iter()
                .find(|candidate| candidate.id == parent_id)
        });
    }
    scissor
}

fn world_transform(display_list: &DisplayList, spatial_id: zeno_scene::SpatialNodeId) -> Transform2D {
    display_list
        .spatial_tree
        .nodes
        .iter()
        .find(|node| node.id == spatial_id)
        .map_or(Transform2D::identity(), |node| node.world_transform)
}

enum ScopeEntry {
    Direct,
    ChildContext(StackingContextId),
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeStep {
    Direct(usize),
    ChildContext(StackingContextId),
}

fn scope_steps(display_list: &DisplayList, parent_context: Option<StackingContextId>) -> Vec<ScopeStep> {
    let mut rendered_children: std::collections::HashSet<StackingContextId> =
        std::collections::HashSet::new();
    let mut steps = Vec::new();
    for (item_index, item) in display_list.items.iter().enumerate() {
        match scope_entry_for_item(display_list, parent_context, item.stacking_context) {
            ScopeEntry::Skip => {}
            ScopeEntry::Direct => steps.push(ScopeStep::Direct(item_index)),
            ScopeEntry::ChildContext(child) => {
                if rendered_children.insert(child) {
                    steps.push(ScopeStep::ChildContext(child));
                }
            }
        }
    }
    steps
}

fn scope_entry_for_item(
    display_list: &DisplayList,
    parent_context: Option<StackingContextId>,
    item_context: Option<StackingContextId>,
) -> ScopeEntry {
    match (parent_context, item_context) {
        (None, None) => ScopeEntry::Direct,
        (Some(parent), Some(current)) if current == parent => ScopeEntry::Direct,
        (scope_parent, Some(current)) => match immediate_child_context(display_list, scope_parent, current) {
            Some(child) => ScopeEntry::ChildContext(child),
            None => ScopeEntry::Skip,
        },
        _ => ScopeEntry::Skip,
    }
}

fn immediate_child_context(
    display_list: &DisplayList,
    parent_context: Option<StackingContextId>,
    mut current: StackingContextId,
) -> Option<StackingContextId> {
    let mut path = vec![current];
    while let Some(parent) = parent_stacking_context(display_list, current) {
        path.push(parent);
        current = parent;
    }
    path.reverse();
    match parent_context {
        None => path.first().copied(),
        Some(parent) => path
            .iter()
            .position(|&id| id == parent)
            .and_then(|index| path.get(index + 1).copied()),
    }
}

fn parent_stacking_context(
    display_list: &DisplayList,
    context_id: StackingContextId,
) -> Option<StackingContextId> {
    let context = display_list
        .stacking_contexts
        .iter()
        .find(|context| context.id == context_id)?;
    let mut current = display_list
        .spatial_tree
        .nodes
        .iter()
        .find(|node| node.id == context.spatial_id)
        .and_then(|node| node.parent);
    while let Some(spatial_id) = current {
        if let Some(parent_context) = display_list
            .stacking_contexts
            .iter()
            .find(|candidate| candidate.spatial_id == spatial_id)
        {
            return Some(parent_context.id);
        }
        current = display_list
            .spatial_tree
            .nodes
            .iter()
            .find(|node| node.id == spatial_id)
            .and_then(|node| node.parent);
    }
    None
}

fn context_bounds(display_list: &DisplayList, context_id: StackingContextId) -> Rect {
    let mut bounds: Option<Rect> = None;
    for item in &display_list.items {
        if item_in_context_subtree(display_list, item.stacking_context, context_id) {
            bounds = Some(match bounds {
                Some(current) => current.union(&item.visual_rect),
                None => item.visual_rect,
            });
        }
    }
    bounds.unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0))
}

fn item_in_context_subtree(
    display_list: &DisplayList,
    item_context: Option<StackingContextId>,
    ancestor: StackingContextId,
) -> bool {
    let mut current = item_context;
    while let Some(id) = current {
        if id == ancestor {
            return true;
        }
        current = parent_stacking_context(display_list, id);
    }
    false
}

fn apply_effect_bounds(bounds: Rect, effects: &[Effect]) -> Rect {
    let mut visual_bounds = bounds;
    for effect in effects {
        match effect {
            Effect::Blur { sigma } => {
                visual_bounds = expand_rect(visual_bounds, sigma * 3.0);
            }
            Effect::DropShadow { dx, dy, blur, .. } => {
                let shadow_bounds = expand_rect(
                    Rect::new(
                        visual_bounds.origin.x + dx,
                        visual_bounds.origin.y + dy,
                        visual_bounds.size.width,
                        visual_bounds.size.height,
                    ),
                    blur * 3.0,
                );
                visual_bounds = visual_bounds.union(&shadow_bounds);
            }
        }
    }
    visual_bounds
}

fn expand_rect(rect: Rect, amount: f32) -> Rect {
    Rect::new(
        rect.origin.x - amount,
        rect.origin.y - amount,
        rect.size.width + amount * 2.0,
        rect.size.height + amount * 2.0,
    )
}

fn scene_blend_mode(mode: BlendMode) -> zeno_scene::SceneBlendMode {
    match mode {
        BlendMode::Normal => zeno_scene::SceneBlendMode::Normal,
        BlendMode::Multiply => zeno_scene::SceneBlendMode::Multiply,
        BlendMode::Screen => zeno_scene::SceneBlendMode::Screen,
    }
}

fn composite_params_for_effects(effects: &[Effect], texture_width: f32, texture_height: f32) -> CompositeParams {
    // Reuse the existing parameter shape by mapping DisplayList effects to the flags/fields.
    let mut blur_sigma = 0.0;
    let mut shadow_blur = 0.0;
    let mut shadow_offset = [0.0, 0.0];
    let mut shadow_color = [0.0, 0.0, 0.0, 0.0];
    let mut flags = 0u32;
    for effect in effects {
        match effect {
            Effect::Blur { sigma } => {
                blur_sigma = *sigma;
                flags |= 1;
            }
            Effect::DropShadow { dx, dy, blur, color } => {
                shadow_blur = *blur;
                shadow_offset = [*dx, *dy];
                shadow_color = [
                    f32::from(color.red) / 255.0,
                    f32::from(color.green) / 255.0,
                    f32::from(color.blue) / 255.0,
                    f32::from(color.alpha) / 255.0,
                ];
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
mod tests {
    use super::{
        clip_scissor, composite_params_for_effects, context_bounds, immediate_child_context,
        scope_steps, ScopeStep,
    };
    use zeno_core::{Color, Rect, Size, Transform2D};
    use zeno_scene::{
        BlendMode, ClipChain, ClipChainId, ClipChainStore, DisplayImage, DisplayItem,
        DisplayItemId, DisplayItemPayload, DisplayList, Effect, SpatialNode, SpatialNodeId,
        SpatialTree, StackingContext, StackingContextId,
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
                    spatial_id: SpatialNodeId(2),
                    opacity: 0.8,
                    blend_mode: BlendMode::Normal,
                    effects: vec![],
                    needs_offscreen: false,
                },
                StackingContext {
                    id: StackingContextId(11),
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
            immediate_child_context(&display_list, Some(StackingContextId(10)), StackingContextId(11)),
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

        let steps = scope_steps(&display_list, None);

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

        let steps = scope_steps(&display_list, Some(StackingContextId(10)));

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
                    Rect::new(24.0, 4.0, 16.0, 12.0),
                    2,
                    2,
                    vec![
                        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
                    ],
                )),
            },
        );

        let steps = scope_steps(&display_list, None);

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
