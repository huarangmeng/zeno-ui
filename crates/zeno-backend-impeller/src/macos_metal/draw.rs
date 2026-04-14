use metal::{
    Buffer, Device, MTLOrigin, MTLPixelFormat, MTLRegion, MTLResourceOptions, MTLSize,
    MTLTextureType, MTLTextureUsage, Texture,
};
use zeno_core::{Color, Rect, Transform2D};
use zeno_scene::Shape;

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

pub(super) fn make_rgba_texture(device: &Device, rgba: &[u8], width: u32, height: u32) -> Texture {
    let descriptor = metal::TextureDescriptor::new();
    descriptor.set_texture_type(MTLTextureType::D2);
    descriptor.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
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
        rgba.as_ptr().cast(),
        (width * 4) as u64,
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
