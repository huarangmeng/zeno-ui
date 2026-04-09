mod shaders;
mod text;

use std::collections::HashMap;

use fontdue::Font;
use metal::{
    Buffer, CommandQueue, CompileOptions, Device, MTLClearColor, MTLBlendFactor, MTLLoadAction,
    MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRegion, MTLResourceOptions, MTLScissorRect,
    MTLSize, MTLStoreAction, MTLTextureType, MTLTextureUsage, MetalDrawableRef,
    RenderPassDescriptor, RenderPipelineDescriptor, RenderPipelineState, Texture,
    TextureDescriptor,
};
use zeno_core::{Color, Point, Rect, Transform2D, ZenoError, ZenoErrorCode};
use zeno_graphics::{DrawCommand, Scene, SceneBlock, SceneClip, SceneLayer, Shape};
use shaders::SHADERS;
use text::{load_system_font, rasterize_text};

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

pub struct MetalSceneRenderer {
    device: Device,
    queue: CommandQueue,
    color_pipeline: RenderPipelineState,
    text_pipeline: RenderPipelineState,
    font: Option<Font>,
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
            font: load_system_font(),
            device,
            queue,
        })
    }

    pub fn render_to_drawable(
        &mut self,
        drawable: &MetalDrawableRef,
        scene: &Scene,
    ) -> Result<(), ZenoError> {
        self.render_to_drawable_with_load(drawable, scene, false)
    }

    pub fn render_to_drawable_with_load(
        &mut self,
        drawable: &MetalDrawableRef,
        scene: &Scene,
        preserve_contents: bool,
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
            );
        } else {
            render_scene_layers(
                &self.device,
                &self.color_pipeline,
                &self.text_pipeline,
                self.font.as_ref(),
                &encoder,
                scene,
                viewport_width,
                viewport_height,
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
    let attachment = descriptor
        .color_attachments()
        .object_at(0)
        .ok_or_else(|| {
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
    let vertex = library
        .get_function("text_vertex", None)
        .map_err(|error| {
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
    let attachment = descriptor
        .color_attachments()
        .object_at(0)
        .ok_or_else(|| {
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
) {
    for command in commands {
        match command {
            DrawCommand::Clear(_) => {}
            DrawCommand::Fill { shape, brush } => {
                let zeno_graphics::Brush::Solid(color) = brush;
                if let Some(vertices) =
                    build_shape_vertices(
                        shape,
                        apply_alpha(*color, opacity_multiplier),
                        viewport_width,
                        viewport_height,
                        transform,
                    )
                {
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
                let Some((mask, width, height, baseline)) =
                    rasterize_text(font, layout.paragraph.text.as_str(), layout.paragraph.font_size)
                else {
                    continue;
                };
                let texture = make_text_texture(device, &mask, width, height);
                let mapped = transform.map_point(Point::new(position.x, position.y - baseline));
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
    font: Option<&Font>,
    encoder: &metal::RenderCommandEncoderRef,
    scene: &Scene,
    viewport_width: f32,
    viewport_height: f32,
) {
    let layers_by_id: HashMap<u64, &SceneLayer> =
        scene.layers.iter().map(|layer| (layer.layer_id, layer)).collect();
    let mut child_layers_by_parent: HashMap<u64, Vec<&SceneLayer>> = HashMap::new();
    let mut blocks_by_layer: HashMap<u64, Vec<&SceneBlock>> = HashMap::new();
    for layer in &scene.layers {
        if let Some(parent_id) = layer.parent_layer_id {
            child_layers_by_parent.entry(parent_id).or_default().push(layer);
        }
    }
    for block in &scene.blocks {
        blocks_by_layer.entry(block.layer_id).or_default().push(block);
    }
    let full_scissor = scissor_for_rect(
        Rect::new(0.0, 0.0, viewport_width, viewport_height),
        viewport_width,
        viewport_height,
    );
    render_layer(
        device,
        color_pipeline,
        text_pipeline,
        font,
        encoder,
        Scene::ROOT_LAYER_ID,
        Transform2D::identity(),
        1.0,
        full_scissor,
        &layers_by_id,
        &child_layers_by_parent,
        &blocks_by_layer,
        viewport_width,
        viewport_height,
    );
    encoder.set_scissor_rect(full_scissor);
}

fn render_layer(
    device: &Device,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    encoder: &metal::RenderCommandEncoderRef,
    layer_id: u64,
    parent_transform: Transform2D,
    parent_opacity: f32,
    parent_scissor: MTLScissorRect,
    layers_by_id: &HashMap<u64, &SceneLayer>,
    child_layers_by_parent: &HashMap<u64, Vec<&SceneLayer>>,
    blocks_by_layer: &HashMap<u64, Vec<&SceneBlock>>,
    viewport_width: f32,
    viewport_height: f32,
) {
    let Some(layer) = layers_by_id.get(&layer_id).copied() else {
        return;
    };
    let combined_transform = parent_transform.then(layer.transform);
    let combined_opacity = parent_opacity * layer.opacity;
    let layer_scissor = layer.clip.map_or(parent_scissor, |clip| {
        intersect_scissor(
            parent_scissor,
            scissor_for_rect(clip_rect(clip, combined_transform), viewport_width, viewport_height),
        )
    });
    encoder.set_scissor_rect(layer_scissor);
    let mut items = Vec::new();
    if let Some(blocks) = blocks_by_layer.get(&layer_id) {
        for block in blocks {
            items.push((block.order, LayerItem::Block(*block)));
        }
    }
    if let Some(children) = child_layers_by_parent.get(&layer_id) {
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
                );
                encoder.set_scissor_rect(layer_scissor);
            }
            LayerItem::Layer(child_layer_id) => render_layer(
                device,
                color_pipeline,
                text_pipeline,
                font,
                encoder,
                child_layer_id,
                combined_transform,
                combined_opacity,
                layer_scissor,
                layers_by_id,
                child_layers_by_parent,
                blocks_by_layer,
                viewport_width,
                viewport_height,
            ),
        }
    }
}

enum LayerItem<'a> {
    Block(&'a SceneBlock),
    Layer(u64),
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
        ([rect.origin.x + rect.size.width, rect.origin.y], [rect.size.width, 0.0]),
        ([rect.origin.x, rect.origin.y + rect.size.height], [0.0, rect.size.height]),
        ([rect.origin.x, rect.origin.y + rect.size.height], [0.0, rect.size.height]),
        ([rect.origin.x + rect.size.width, rect.origin.y], [rect.size.width, 0.0]),
        (
            [rect.origin.x + rect.size.width, rect.origin.y + rect.size.height],
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

fn clear_color_for_scene(scene: &Scene) -> MTLClearColor {
    let clear = scene
        .clear_color
        .or_else(|| scene.commands.iter().find_map(|command| match command {
            DrawCommand::Clear(color) => Some(*color),
            _ => None,
        }))
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

fn apply_alpha(color: Color, opacity_multiplier: f32) -> Color {
    let alpha = ((f32::from(color.alpha) * opacity_multiplier).clamp(0.0, 255.0)).round() as u8;
    Color::rgba(color.red, color.green, color.blue, alpha)
}
