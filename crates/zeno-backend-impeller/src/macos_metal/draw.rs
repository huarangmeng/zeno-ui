use fontdue::Font;
use metal::{
    Buffer, Device, MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRegion, MTLResourceOptions,
    MTLSize, MTLTextureType, MTLTextureUsage, RenderPipelineState, Texture,
};
use zeno_core::{Color, Point, Rect, Transform2D};
use zeno_scene::{DrawCommand, Scene, Shape};
use zeno_text::GlyphRasterCache;

use super::text::rasterize_layout;

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub(super) struct ColorVertex {
    pub clip_position: [f32; 2],
    pub local_position: [f32; 2],
    pub size: [f32; 2],
    pub _padding0: [f32; 2],
    pub color: [f32; 4],
    pub radius: f32,
    pub _padding1: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct TextVertex {
    pub clip_position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct CompositeVertex {
    pub clip_position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

// 该模块只负责把 Scene 的绘制命令翻译成 GPU 顶点与纹理资源。
pub(super) fn draw_commands(
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
    glyph_cache: &GlyphRasterCache,
) {
    for command in commands {
        match command {
            DrawCommand::Clear(_) => {}
            DrawCommand::Fill { shape, brush } => {
                let zeno_scene::Brush::Solid(color) = brush;
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
                        Some(glyph_cache.get_or_rasterize(font, glyph_id, glyph, font_size))
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

pub(super) fn build_shape_vertices(
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

pub(super) fn build_quad_vertices(
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

pub(super) fn build_text_vertices(
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

pub(super) fn build_composite_vertices(
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

pub(super) fn make_text_texture(device: &Device, alpha: &[u8], width: u32, height: u32) -> Texture {
    let descriptor = metal::TextureDescriptor::new();
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

pub(super) fn make_offscreen_texture(device: &Device, width: u64, height: u64) -> Texture {
    let descriptor = metal::TextureDescriptor::new();
    descriptor.set_texture_type(MTLTextureType::D2);
    descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    descriptor.set_width(width);
    descriptor.set_height(height);
    descriptor.set_usage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
    device.new_texture(&descriptor)
}

pub(super) fn clear_color_for_scene(scene: &Scene) -> metal::MTLClearColor {
    let clear = scene
        .clear_color
        .or_else(|| {
            scene.commands.iter().find_map(|command| match command {
                DrawCommand::Clear(color) => Some(*color),
                _ => None,
            })
        })
        .unwrap_or(Color::WHITE);
    metal::MTLClearColor::new(
        f64::from(clear.red) / 255.0,
        f64::from(clear.green) / 255.0,
        f64::from(clear.blue) / 255.0,
        f64::from(clear.alpha) / 255.0,
    )
}

pub(super) fn color_to_f32(color: Color) -> [f32; 4] {
    [
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        f32::from(color.alpha) / 255.0,
    ]
}

pub(super) fn to_clip_space(x: f32, y: f32, viewport_width: f32, viewport_height: f32) -> [f32; 2] {
    [
        (x / viewport_width) * 2.0 - 1.0,
        1.0 - (y / viewport_height) * 2.0,
    ]
}

pub(super) fn new_buffer<T>(device: &Device, values: &[T]) -> Buffer {
    device.new_buffer_with_data(
        values.as_ptr().cast(),
        std::mem::size_of_val(values) as u64,
        MTLResourceOptions::CPUCacheModeDefaultCache,
    )
}

pub(super) fn apply_alpha(color: Color, opacity_multiplier: f32) -> Color {
    let alpha = ((f32::from(color.alpha) * opacity_multiplier).clamp(0.0, 255.0)).round() as u8;
    Color::rgba(color.red, color.green, color.blue, alpha)
}
