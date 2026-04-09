mod shaders;
mod text;

use std::collections::HashMap;

use fontdue::Font;
use metal::{
    Buffer, CommandBufferRef, CommandQueue, CompileOptions, Device, MTLBlendFactor, MTLClearColor,
    MTLLoadAction, MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRegion, MTLResourceOptions,
    MTLScissorRect, MTLSize, MTLStoreAction, MTLTextureType, MTLTextureUsage, MetalDrawableRef,
    RenderPassDescriptor, RenderPipelineDescriptor, RenderPipelineState, Texture,
    TextureDescriptor,
};
use shaders::SHADERS;
use text::{
    CachedGlyph, GlyphCacheKey, glyph_cache_key, load_system_font, rasterize_glyph,
    rasterize_layout,
};
use zeno_core::{Color, Point, Rect, Transform2D, ZenoError, ZenoErrorCode};
use zeno_graphics::{
    DrawCommand, Scene, SceneBlendMode, SceneBlock, SceneClip, SceneEffect, SceneLayer, Shape,
};

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct ColorVertex {
    clip_position: [f32; 2],
    local_position: [f32; 2],
    size: [f32; 2],
    _padding0: [f32; 2],
    color: [f32; 4],
    radius: f32,
    _padding1: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TextVertex {
    clip_position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CompositeVertex {
    clip_position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct CompositeParams {
    inv_texture_size: [f32; 2],
    blur_sigma: f32,
    shadow_blur: f32,
    shadow_offset: [f32; 2],
    shadow_color: [f32; 4],
    flags: u32,
    _padding: [u32; 3],
}

pub struct MetalSceneRenderer {
    device: Device,
    queue: CommandQueue,
    color_pipeline: RenderPipelineState,
    text_pipeline: RenderPipelineState,
    composite_pipeline: RenderPipelineState,
    composite_multiply_pipeline: RenderPipelineState,
    composite_screen_pipeline: RenderPipelineState,
    font: Option<Font>,
    glyph_cache: HashMap<GlyphCacheKey, CachedGlyph>,
}

impl MetalSceneRenderer {
    pub fn new(device: Device, queue: CommandQueue) -> Result<Self, ZenoError> {
        let library = device
            .new_library_with_source(SHADERS, &CompileOptions::new())
            .map_err(|error| {
                ZenoError::invalid_configuration(
                    ZenoErrorCode::BackendImpellerShaderCompileFailed,
                    "backend.impeller",
                    "compile_shaders",
                    error.to_string(),
                )
            })?;

        Ok(Self {
            color_pipeline: create_color_pipeline(&device, &library)?,
            text_pipeline: create_text_pipeline(&device, &library)?,
            composite_pipeline: create_composite_pipeline(
                &device,
                &library,
                SceneBlendMode::Normal,
            )?,
            composite_multiply_pipeline: create_composite_pipeline(
                &device,
                &library,
                SceneBlendMode::Multiply,
            )?,
            composite_screen_pipeline: create_composite_pipeline(
                &device,
                &library,
                SceneBlendMode::Screen,
            )?,
            font: load_system_font(),
            glyph_cache: HashMap::new(),
            device,
            queue,
        })
    }

    pub fn render_to_drawable(
        &mut self,
        drawable: &MetalDrawableRef,
        scene: &Scene,
    ) -> Result<(), ZenoError> {
        self.render_to_drawable_region_with_load(drawable, scene, false, None)
    }

    pub fn render_to_drawable_with_load(
        &mut self,
        drawable: &MetalDrawableRef,
        scene: &Scene,
        preserve_contents: bool,
    ) -> Result<(), ZenoError> {
        self.render_to_drawable_region_with_load(drawable, scene, preserve_contents, None)
    }

    pub fn render_to_drawable_region_with_load(
        &mut self,
        drawable: &MetalDrawableRef,
        scene: &Scene,
        preserve_contents: bool,
        dirty_bounds: Option<Rect>,
    ) -> Result<(), ZenoError> {
        let render_pass = RenderPassDescriptor::new();
        let attachment = render_pass
            .color_attachments()
            .object_at(0)
            .ok_or_else(|| {
                ZenoError::invalid_configuration(
                    ZenoErrorCode::BackendImpellerRenderPassAttachmentMissing,
                    "backend.impeller",
                    "render_to_drawable",
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
            attachment.set_clear_color(clear_color_for_scene(scene));
        }

        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_render_command_encoder(&render_pass);
        let viewport_width = scene.size.width.max(1.0);
        let viewport_height = scene.size.height.max(1.0);
        let root_scissor = effective_root_scissor(dirty_bounds, viewport_width, viewport_height);
        encoder.set_scissor_rect(root_scissor);

        if scene.blocks.is_empty() {
            draw_commands(
                &self.device,
                &self.color_pipeline,
                &self.text_pipeline,
                self.font.as_ref(),
                &encoder,
                &scene.commands,
                viewport_width,
                viewport_height,
                Transform2D::identity(),
                1.0,
                &mut self.glyph_cache,
            );
        } else {
            render_scene_layers(
                &self.device,
                &self.color_pipeline,
                &self.text_pipeline,
                &self.composite_pipeline,
                &self.composite_multiply_pipeline,
                &self.composite_screen_pipeline,
                self.font.as_ref(),
                &command_buffer,
                &encoder,
                scene,
                root_scissor,
                viewport_width,
                viewport_height,
                &mut self.glyph_cache,
            );
        }

        encoder.end_encoding();
        command_buffer.present_drawable(drawable);
        command_buffer.commit();
        Ok(())
    }
}

fn create_color_pipeline(
    device: &Device,
    library: &metal::Library,
) -> Result<RenderPipelineState, ZenoError> {
    let descriptor = RenderPipelineDescriptor::new();
    let vertex = library
        .get_function("color_vertex", None)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerColorPipelineFunctionMissing,
                "backend.impeller",
                "create_color_pipeline",
                error.to_string(),
            )
        })?;
    let fragment = library
        .get_function("color_fragment", None)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerColorPipelineFunctionMissing,
                "backend.impeller",
                "create_color_pipeline",
                error.to_string(),
            )
        })?;
    descriptor.set_vertex_function(Some(&vertex));
    descriptor.set_fragment_function(Some(&fragment));
    let attachment = descriptor.color_attachments().object_at(0).ok_or_else(|| {
        ZenoError::invalid_configuration(
            ZenoErrorCode::BackendImpellerColorPipelineAttachmentMissing,
            "backend.impeller",
            "create_color_pipeline",
            "missing color attachment",
        )
    })?;
    attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    attachment.set_blending_enabled(true);
    attachment.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    attachment.set_source_alpha_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    device
        .new_render_pipeline_state(&descriptor)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerColorPipelineStateCreateFailed,
                "backend.impeller",
                "create_color_pipeline",
                error.to_string(),
            )
        })
}

fn create_text_pipeline(
    device: &Device,
    library: &metal::Library,
) -> Result<RenderPipelineState, ZenoError> {
    let descriptor = RenderPipelineDescriptor::new();
    let vertex = library.get_function("text_vertex", None).map_err(|error| {
        ZenoError::invalid_configuration(
            ZenoErrorCode::BackendImpellerTextPipelineFunctionMissing,
            "backend.impeller",
            "create_text_pipeline",
            error.to_string(),
        )
    })?;
    let fragment = library
        .get_function("text_fragment", None)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerTextPipelineFunctionMissing,
                "backend.impeller",
                "create_text_pipeline",
                error.to_string(),
            )
        })?;
    descriptor.set_vertex_function(Some(&vertex));
    descriptor.set_fragment_function(Some(&fragment));
    let attachment = descriptor.color_attachments().object_at(0).ok_or_else(|| {
        ZenoError::invalid_configuration(
            ZenoErrorCode::BackendImpellerTextPipelineAttachmentMissing,
            "backend.impeller",
            "create_text_pipeline",
            "missing color attachment",
        )
    })?;
    attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    attachment.set_blending_enabled(true);
    attachment.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    attachment.set_source_alpha_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    device
        .new_render_pipeline_state(&descriptor)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerTextPipelineStateCreateFailed,
                "backend.impeller",
                "create_text_pipeline",
                error.to_string(),
            )
        })
}

fn create_composite_pipeline(
    device: &Device,
    library: &metal::Library,
    blend_mode: SceneBlendMode,
) -> Result<RenderPipelineState, ZenoError> {
    let descriptor = RenderPipelineDescriptor::new();
    let vertex = library
        .get_function("composite_vertex", None)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerCompositePipelineFunctionMissing,
                "backend.impeller",
                "create_composite_pipeline",
                error.to_string(),
            )
        })?;
    let fragment = library
        .get_function("composite_fragment", None)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerCompositePipelineFunctionMissing,
                "backend.impeller",
                "create_composite_pipeline",
                error.to_string(),
            )
        })?;
    descriptor.set_vertex_function(Some(&vertex));
    descriptor.set_fragment_function(Some(&fragment));
    let attachment = descriptor.color_attachments().object_at(0).ok_or_else(|| {
        ZenoError::invalid_configuration(
            ZenoErrorCode::BackendImpellerCompositePipelineAttachmentMissing,
            "backend.impeller",
            "create_composite_pipeline",
            "missing color attachment",
        )
    })?;
    attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    attachment.set_blending_enabled(true);
    match blend_mode {
        SceneBlendMode::Normal => {
            attachment.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
            attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
            attachment.set_source_alpha_blend_factor(MTLBlendFactor::SourceAlpha);
            attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        }
        SceneBlendMode::Multiply => {
            attachment.set_source_rgb_blend_factor(MTLBlendFactor::DestinationColor);
            attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
            attachment.set_source_alpha_blend_factor(MTLBlendFactor::One);
            attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        }
        SceneBlendMode::Screen => {
            attachment.set_source_rgb_blend_factor(MTLBlendFactor::One);
            attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceColor);
            attachment.set_source_alpha_blend_factor(MTLBlendFactor::One);
            attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        }
    }
    device
        .new_render_pipeline_state(&descriptor)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerCompositePipelineStateCreateFailed,
                "backend.impeller",
                "create_composite_pipeline",
                error.to_string(),
            )
        })
}

fn draw_commands(
    device: &Device,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    encoder: &metal::RenderCommandEncoderRef,
    commands: &[DrawCommand],
    viewport_width: f32,
    viewport_height: f32,
    transform: Transform2D,
    opacity_multiplier: f32,
    glyph_cache: &mut HashMap<GlyphCacheKey, CachedGlyph>,
) {
    for command in commands {
        match command {
            DrawCommand::Clear(_) => {}
            DrawCommand::Fill { shape, brush } => {
                let zeno_graphics::Brush::Solid(color) = brush;
                if let Some(vertices) = build_shape_vertices(
                    shape,
                    apply_alpha(*color, opacity_multiplier),
                    viewport_width,
                    viewport_height,
                    transform,
                ) {
                    let buffer = new_buffer(device, &vertices);
                    encoder.set_render_pipeline_state(color_pipeline);
                    encoder.set_vertex_buffer(0, Some(&buffer), 0);
                    encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
                }
            }
            DrawCommand::Stroke { .. } => {}
            DrawCommand::Text {
                position,
                layout,
                color,
            } => {
                let Some(font) = font else {
                    continue;
                };
                let Some((mask, width, height)) =
                    rasterize_layout(layout, |glyph_id, glyph, font_size| {
                        let key = glyph_cache_key(glyph_id, font_size);
                        if let Some(cached) = glyph_cache.get(&key) {
                            return Some(cached.clone());
                        }
                        let cached = rasterize_glyph(font, glyph_id, glyph, font_size);
                        glyph_cache.insert(key, cached.clone());
                        Some(cached)
                    })
                else {
                    continue;
                };
                let texture = make_text_texture(device, &mask, width, height);
                let mapped =
                    transform.map_point(Point::new(position.x, position.y - layout.metrics.ascent));
                let vertices = build_text_vertices(
                    mapped.x,
                    mapped.y,
                    width as f32,
                    height as f32,
                    apply_alpha(*color, opacity_multiplier),
                    viewport_width,
                    viewport_height,
                );
                let buffer = new_buffer(device, &vertices);
                encoder.set_render_pipeline_state(text_pipeline);
                encoder.set_vertex_buffer(0, Some(&buffer), 0);
                encoder.set_fragment_texture(0, Some(&texture));
                encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
            }
        }
    }
}

fn render_scene_layers(
    device: &Device,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    command_buffer: &CommandBufferRef,
    encoder: &metal::RenderCommandEncoderRef,
    scene: &Scene,
    root_scissor: MTLScissorRect,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &mut HashMap<GlyphCacheKey, CachedGlyph>,
) {
    let layers_by_id: HashMap<u64, &SceneLayer> = scene
        .layers
        .iter()
        .map(|layer| (layer.layer_id, layer))
        .collect();
    let mut child_layers_by_parent: HashMap<u64, Vec<&SceneLayer>> = HashMap::new();
    let mut blocks_by_layer: HashMap<u64, Vec<&SceneBlock>> = HashMap::new();
    for layer in &scene.layers {
        if let Some(parent_id) = layer.parent_layer_id {
            child_layers_by_parent
                .entry(parent_id)
                .or_default()
                .push(layer);
        }
    }
    for block in &scene.blocks {
        blocks_by_layer
            .entry(block.layer_id)
            .or_default()
            .push(block);
    }
    let Some(root_layer) = layers_by_id.get(&Scene::ROOT_LAYER_ID).copied() else {
        return;
    };
    render_layer(
        device,
        color_pipeline,
        text_pipeline,
        composite_pipeline,
        composite_multiply_pipeline,
        composite_screen_pipeline,
        font,
        command_buffer,
        encoder,
        root_layer,
        Transform2D::identity(),
        1.0,
        root_scissor,
        &layers_by_id,
        &child_layers_by_parent,
        &blocks_by_layer,
        viewport_width,
        viewport_height,
        glyph_cache,
    );
    encoder.set_scissor_rect(root_scissor);
}

fn render_layer(
    device: &Device,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    command_buffer: &CommandBufferRef,
    encoder: &metal::RenderCommandEncoderRef,
    layer: &SceneLayer,
    combined_transform: Transform2D,
    combined_opacity: f32,
    parent_scissor: MTLScissorRect,
    layers_by_id: &HashMap<u64, &SceneLayer>,
    child_layers_by_parent: &HashMap<u64, Vec<&SceneLayer>>,
    blocks_by_layer: &HashMap<u64, Vec<&SceneBlock>>,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &mut HashMap<GlyphCacheKey, CachedGlyph>,
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
    if let Some(blocks) = blocks_by_layer.get(&layer.layer_id) {
        for block in blocks {
            items.push((block.order, LayerItem::Block(*block)));
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
            LayerItem::Block(block) => {
                let block_transform = combined_transform.then(block.transform);
                let block_scissor = block.clip.map_or(layer_scissor, |clip| {
                    intersect_scissor(
                        layer_scissor,
                        scissor_for_rect(
                            clip_rect(clip, block_transform),
                            viewport_width,
                            viewport_height,
                        ),
                    )
                });
                encoder.set_scissor_rect(block_scissor);
                draw_commands(
                    device,
                    color_pipeline,
                    text_pipeline,
                    font,
                    encoder,
                    &block.commands,
                    viewport_width,
                    viewport_height,
                    block_transform,
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
                        color_pipeline,
                        text_pipeline,
                        composite_pipeline,
                        composite_multiply_pipeline,
                        composite_screen_pipeline,
                        font,
                        command_buffer,
                        encoder,
                        child_layer,
                        child_transform,
                        child_opacity,
                        layer_scissor,
                        layers_by_id,
                        child_layers_by_parent,
                        blocks_by_layer,
                        viewport_width,
                        viewport_height,
                        glyph_cache,
                    );
                    encoder.set_scissor_rect(layer_scissor);
                } else {
                    render_layer(
                        device,
                        color_pipeline,
                        text_pipeline,
                        composite_pipeline,
                        composite_multiply_pipeline,
                        composite_screen_pipeline,
                        font,
                        command_buffer,
                        encoder,
                        child_layer,
                        child_transform,
                        child_opacity,
                        layer_scissor,
                        layers_by_id,
                        child_layers_by_parent,
                        blocks_by_layer,
                        viewport_width,
                        viewport_height,
                        glyph_cache,
                    );
                }
            }
        }
    }
}

fn render_offscreen_layer(
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
        scissor_for_rect(composite_rect, parent_viewport_width, parent_viewport_height),
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

enum LayerItem<'a> {
    Block(&'a SceneBlock),
    Layer(u64),
}

fn should_render_offscreen(layer: &SceneLayer) -> bool {
    layer.layer_id != Scene::ROOT_LAYER_ID
        && (layer.offscreen
            || layer.blend_mode != SceneBlendMode::Normal
            || !layer.effects.is_empty())
}

fn build_shape_vertices(
    shape: &Shape,
    color: Color,
    viewport_width: f32,
    viewport_height: f32,
    transform: Transform2D,
) -> Option<Vec<ColorVertex>> {
    let (rect, radius) = match shape {
        Shape::Rect(rect) => (*rect, 0.0),
        Shape::RoundedRect { rect, radius } => (*rect, *radius),
        Shape::Circle { .. } => return None,
    };
    Some(build_quad_vertices(
        transform.map_rect(rect),
        radius,
        color,
        viewport_width,
        viewport_height,
    ))
}

fn build_quad_vertices(
    rect: Rect,
    radius: f32,
    color: Color,
    viewport_width: f32,
    viewport_height: f32,
) -> Vec<ColorVertex> {
    let rgba = color_to_f32(color);
    [
        ([rect.origin.x, rect.origin.y], [0.0, 0.0]),
        (
            [rect.origin.x + rect.size.width, rect.origin.y],
            [rect.size.width, 0.0],
        ),
        (
            [rect.origin.x, rect.origin.y + rect.size.height],
            [0.0, rect.size.height],
        ),
        (
            [rect.origin.x, rect.origin.y + rect.size.height],
            [0.0, rect.size.height],
        ),
        (
            [rect.origin.x + rect.size.width, rect.origin.y],
            [rect.size.width, 0.0],
        ),
        (
            [
                rect.origin.x + rect.size.width,
                rect.origin.y + rect.size.height,
            ],
            [rect.size.width, rect.size.height],
        ),
    ]
    .into_iter()
    .map(|(position, local)| ColorVertex {
        clip_position: to_clip_space(position[0], position[1], viewport_width, viewport_height),
        local_position: local,
        size: [rect.size.width, rect.size.height],
        _padding0: [0.0, 0.0],
        color: rgba,
        radius,
        _padding1: [0.0, 0.0, 0.0],
    })
    .collect()
}

fn build_text_vertices(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Color,
    viewport_width: f32,
    viewport_height: f32,
) -> Vec<TextVertex> {
    let rgba = color_to_f32(color);
    [
        ([x, y], [0.0, 0.0]),
        ([x + width, y], [1.0, 0.0]),
        ([x, y + height], [0.0, 1.0]),
        ([x, y + height], [0.0, 1.0]),
        ([x + width, y], [1.0, 0.0]),
        ([x + width, y + height], [1.0, 1.0]),
    ]
    .into_iter()
    .map(|(position, uv)| TextVertex {
        clip_position: to_clip_space(position[0], position[1], viewport_width, viewport_height),
        uv,
        color: rgba,
    })
    .collect()
}

fn build_composite_vertices(
    rect: Rect,
    opacity: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> Vec<CompositeVertex> {
    let color = [1.0, 1.0, 1.0, opacity.clamp(0.0, 1.0)];
    [
        ([rect.origin.x, rect.origin.y], [0.0, 0.0]),
        ([rect.origin.x + rect.size.width, rect.origin.y], [1.0, 0.0]),
        (
            [rect.origin.x, rect.origin.y + rect.size.height],
            [0.0, 1.0],
        ),
        (
            [rect.origin.x, rect.origin.y + rect.size.height],
            [0.0, 1.0],
        ),
        ([rect.origin.x + rect.size.width, rect.origin.y], [1.0, 0.0]),
        (
            [
                rect.origin.x + rect.size.width,
                rect.origin.y + rect.size.height,
            ],
            [1.0, 1.0],
        ),
    ]
    .into_iter()
    .map(|(position, uv)| CompositeVertex {
        clip_position: to_clip_space(position[0], position[1], viewport_width, viewport_height),
        uv,
        color,
    })
    .collect()
}

fn make_text_texture(device: &Device, alpha: &[u8], width: u32, height: u32) -> Texture {
    let descriptor = TextureDescriptor::new();
    descriptor.set_texture_type(MTLTextureType::D2);
    descriptor.set_pixel_format(MTLPixelFormat::R8Unorm);
    descriptor.set_width(width as u64);
    descriptor.set_height(height as u64);
    descriptor.set_usage(MTLTextureUsage::ShaderRead);
    let texture = device.new_texture(&descriptor);
    texture.replace_region(
        MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width: width as u64,
                height: height as u64,
                depth: 1,
            },
        },
        0,
        alpha.as_ptr().cast(),
        width as u64,
    );
    texture
}

fn make_offscreen_texture(device: &Device, width: u64, height: u64) -> Texture {
    let descriptor = TextureDescriptor::new();
    descriptor.set_texture_type(MTLTextureType::D2);
    descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    descriptor.set_width(width);
    descriptor.set_height(height);
    descriptor.set_usage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
    device.new_texture(&descriptor)
}

fn draw_composited_texture(
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
    let vertices = build_composite_vertices(rect, opacity, viewport_width, viewport_height);
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

fn composite_pipeline_for_blend<'a>(
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

fn composite_params(
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

fn local_effect_bounds(layer: &SceneLayer) -> Rect {
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

fn expand_rect(rect: Rect, amount: f32) -> Rect {
    Rect::new(
        rect.origin.x - amount,
        rect.origin.y - amount,
        rect.size.width + amount * 2.0,
        rect.size.height + amount * 2.0,
    )
}

fn clear_color_for_scene(scene: &Scene) -> MTLClearColor {
    let clear = scene
        .clear_color
        .or_else(|| {
            scene.commands.iter().find_map(|command| match command {
                DrawCommand::Clear(color) => Some(*color),
                _ => None,
            })
        })
        .unwrap_or(Color::WHITE);
    MTLClearColor::new(
        f64::from(clear.red) / 255.0,
        f64::from(clear.green) / 255.0,
        f64::from(clear.blue) / 255.0,
        f64::from(clear.alpha) / 255.0,
    )
}

fn color_to_f32(color: Color) -> [f32; 4] {
    [
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        f32::from(color.alpha) / 255.0,
    ]
}

fn to_clip_space(x: f32, y: f32, viewport_width: f32, viewport_height: f32) -> [f32; 2] {
    [
        (x / viewport_width) * 2.0 - 1.0,
        1.0 - (y / viewport_height) * 2.0,
    ]
}

fn new_buffer<T>(device: &Device, values: &[T]) -> Buffer {
    device.new_buffer_with_data(
        values.as_ptr().cast(),
        std::mem::size_of_val(values) as u64,
        MTLResourceOptions::CPUCacheModeDefaultCache,
    )
}

fn clip_rect(clip: SceneClip, transform: Transform2D) -> Rect {
    match clip {
        SceneClip::Rect(rect) => transform.map_rect(rect),
        SceneClip::RoundedRect { rect, .. } => transform.map_rect(rect),
    }
}

fn scissor_for_rect(rect: Rect, viewport_width: f32, viewport_height: f32) -> MTLScissorRect {
    let min_x = rect.origin.x.max(0.0).floor() as u64;
    let min_y = rect.origin.y.max(0.0).floor() as u64;
    let max_x = (rect.origin.x + rect.size.width)
        .min(viewport_width)
        .max(min_x as f32)
        .ceil() as u64;
    let max_y = (rect.origin.y + rect.size.height)
        .min(viewport_height)
        .max(min_y as f32)
        .ceil() as u64;
    MTLScissorRect {
        x: min_x,
        y: min_y,
        width: max_x.saturating_sub(min_x),
        height: max_y.saturating_sub(min_y),
    }
}

fn effective_root_scissor(
    dirty_bounds: Option<Rect>,
    viewport_width: f32,
    viewport_height: f32,
) -> MTLScissorRect {
    dirty_bounds.map_or_else(
        || {
            scissor_for_rect(
                Rect::new(0.0, 0.0, viewport_width, viewport_height),
                viewport_width,
                viewport_height,
            )
        },
        |bounds| scissor_for_rect(bounds, viewport_width, viewport_height),
    )
}

fn intersect_scissor(a: MTLScissorRect, b: MTLScissorRect) -> MTLScissorRect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    MTLScissorRect {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}

fn rect_from_scissor(scissor: MTLScissorRect) -> Rect {
    Rect::new(
        scissor.x as f32,
        scissor.y as f32,
        scissor.width as f32,
        scissor.height as f32,
    )
}

fn rect_intersection(a: Rect, b: Rect) -> Option<Rect> {
    if !a.intersects(&b) {
        return None;
    }
    let left = a.origin.x.max(b.origin.x);
    let top = a.origin.y.max(b.origin.y);
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    Some(Rect::new(left, top, right - left, bottom - top))
}

fn inverse_map_rect(transform: Transform2D, rect: Rect) -> Option<Rect> {
    let determinant = transform.m11 * transform.m22 - transform.m21 * transform.m12;
    if determinant.abs() <= f32::EPSILON {
        return None;
    }
    let inverse = Transform2D {
        m11: transform.m22 / determinant,
        m12: -transform.m12 / determinant,
        m21: -transform.m21 / determinant,
        m22: transform.m11 / determinant,
        tx: (transform.m21 * transform.ty - transform.m22 * transform.tx) / determinant,
        ty: (transform.m12 * transform.tx - transform.m11 * transform.ty) / determinant,
    };
    Some(inverse.map_rect(rect))
}

fn apply_alpha(color: Color, opacity_multiplier: f32) -> Color {
    let alpha = ((f32::from(color.alpha) * opacity_multiplier).clamp(0.0, 255.0)).round() as u8;
    Color::rgba(color.red, color.green, color.blue, alpha)
}

#[cfg(test)]
mod tests {
    use super::{
        build_composite_vertices, composite_params, effective_root_scissor, inverse_map_rect,
        local_effect_bounds, rect_from_scissor, should_render_offscreen,
    };
    use zeno_core::{Color, Rect, Size, Transform2D};
    use zeno_graphics::{Scene, SceneBlendMode, SceneEffect, SceneLayer};

    #[test]
    fn offscreen_layer_policy_skips_root_and_keeps_explicit_offscreen_layers() {
        let root = SceneLayer::root(Size::new(200.0, 100.0));
        let child = SceneLayer::new(
            10,
            10,
            Some(Scene::ROOT_LAYER_ID),
            1,
            Rect::new(0.0, 0.0, 40.0, 30.0),
            Rect::new(0.0, 0.0, 40.0, 30.0),
            Transform2D::identity(),
            None,
            0.5,
            SceneBlendMode::Normal,
            Vec::new(),
            true,
        );

        assert!(!should_render_offscreen(&root));
        assert!(should_render_offscreen(&child));
    }

    #[test]
    fn composite_vertices_preserve_opacity_in_vertex_color() {
        let vertices =
            build_composite_vertices(Rect::new(10.0, 20.0, 30.0, 40.0), 0.25, 100.0, 100.0);

        assert_eq!(vertices.len(), 6);
        assert_eq!(vertices[0].color, [1.0, 1.0, 1.0, 0.25]);
        assert_eq!(vertices[0].uv, [0.0, 0.0]);
        assert_eq!(vertices[5].uv, [1.0, 1.0]);
    }

    #[test]
    fn effect_bounds_and_params_include_blur_and_shadow() {
        let layer = SceneLayer::new(
            10,
            10,
            Some(Scene::ROOT_LAYER_ID),
            1,
            Rect::new(0.0, 0.0, 40.0, 30.0),
            Rect::new(-18.0, -18.0, 76.0, 66.0),
            Transform2D::identity(),
            None,
            1.0,
            SceneBlendMode::Screen,
            vec![
                SceneEffect::Blur { sigma: 2.0 },
                SceneEffect::DropShadow {
                    dx: 4.0,
                    dy: 6.0,
                    blur: 3.0,
                    color: Color::rgba(10, 20, 30, 128),
                },
            ],
            true,
        );
        let bounds = local_effect_bounds(&layer);
        let params = composite_params(&layer, 76.0, 66.0);

        let blur_bounds = Rect::new(-6.0, -6.0, 52.0, 42.0);
        let shadow_bounds = Rect::new(-11.0, -9.0, 70.0, 60.0);
        assert_eq!(bounds, blur_bounds.union(&shadow_bounds));
        assert_eq!(params.flags, 3);
        assert_eq!(params.shadow_offset, [4.0, 6.0]);
        assert_eq!(
            params.shadow_color,
            [10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 128.0 / 255.0]
        );
    }

    #[test]
    fn root_scissor_uses_dirty_bounds_when_present() {
        let full = effective_root_scissor(None, 200.0, 100.0);
        let dirty = effective_root_scissor(Some(Rect::new(10.4, 20.2, 30.1, 40.6)), 200.0, 100.0);

        assert_eq!(full.x, 0);
        assert_eq!(full.y, 0);
        assert_eq!(full.width, 200);
        assert_eq!(full.height, 100);
        assert_eq!(dirty.x, 10);
        assert_eq!(dirty.y, 20);
        assert_eq!(dirty.width, 31);
        assert_eq!(dirty.height, 41);
    }

    #[test]
    fn inverse_map_rect_restores_translated_and_scaled_bounds() {
        let transform = Transform2D::translation(30.0, 10.0).then(Transform2D::scale(2.0, 4.0));
        let local = Rect::new(5.0, 2.0, 10.0, 10.0);
        let world = transform.map_rect(local);
        let local = inverse_map_rect(transform, world).expect("invertible transform");

        assert_eq!(local, Rect::new(5.0, 2.0, 10.0, 10.0));
    }

    #[test]
    fn rect_from_scissor_matches_scissor_extent() {
        let rect = rect_from_scissor(metal::MTLScissorRect {
            x: 12,
            y: 8,
            width: 40,
            height: 24,
        });

        assert_eq!(rect, Rect::new(12.0, 8.0, 40.0, 24.0));
    }
}
