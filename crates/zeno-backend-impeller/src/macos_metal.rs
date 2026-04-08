mod shaders;
mod text;

use fontdue::Font;
use metal::{
    Buffer, CommandQueue, CompileOptions, Device, MTLClearColor, MTLBlendFactor, MTLLoadAction,
    MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRegion, MTLResourceOptions, MTLSize,
    MTLStoreAction, MTLTextureType, MTLTextureUsage, MetalDrawableRef, RenderPassDescriptor,
    RenderPipelineDescriptor, RenderPipelineState, Texture, TextureDescriptor,
};
use zeno_core::{Color, Rect, ZenoError, ZenoErrorCode};
use zeno_graphics::{DrawCommand, Scene, Shape};
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
        attachment.set_load_action(MTLLoadAction::Clear);
        attachment.set_store_action(MTLStoreAction::Store);
        attachment.set_clear_color(clear_color_for_scene(scene));

        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_render_command_encoder(&render_pass);
        let viewport_width = scene.size.width.max(1.0);
        let viewport_height = scene.size.height.max(1.0);

        for command in &scene.commands {
            match command {
                DrawCommand::Clear(_) => {}
                DrawCommand::Fill { shape, brush } => {
                    let zeno_graphics::Brush::Solid(color) = brush;
                    if let Some(vertices) =
                        build_shape_vertices(shape, *color, viewport_width, viewport_height)
                    {
                        let buffer = new_buffer(&self.device, &vertices);
                        encoder.set_render_pipeline_state(&self.color_pipeline);
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
                    let Some(font) = self.font.as_ref() else {
                        continue;
                    };
                    let Some((mask, width, height, baseline)) =
                        rasterize_text(font, layout.paragraph.text.as_str(), layout.paragraph.font_size)
                    else {
                        continue;
                    };
                    let texture = make_text_texture(&self.device, &mask, width, height);
                    let vertices = build_text_vertices(
                        position.x,
                        position.y - baseline,
                        width as f32,
                        height as f32,
                        *color,
                        viewport_width,
                        viewport_height,
                    );
                    let buffer = new_buffer(&self.device, &vertices);
                    encoder.set_render_pipeline_state(&self.text_pipeline);
                    encoder.set_vertex_buffer(0, Some(&buffer), 0);
                    encoder.set_fragment_texture(0, Some(&texture));
                    encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
                }
            }
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

fn build_shape_vertices(
    shape: &Shape,
    color: Color,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<Vec<ColorVertex>> {
    let (rect, radius) = match shape {
        Shape::Rect(rect) => (*rect, 0.0),
        Shape::RoundedRect { rect, radius } => (*rect, *radius),
        Shape::Circle { .. } => return None,
    };
    Some(build_quad_vertices(rect, radius, color, viewport_width, viewport_height))
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
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Clear(color) => Some(*color),
            _ => None,
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
