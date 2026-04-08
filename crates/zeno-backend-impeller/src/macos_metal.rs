use font_kit::source::SystemSource;
use fontdue::Font;
use metal::{
    Buffer, CommandQueue, CompileOptions, Device, MTLClearColor, MTLBlendFactor, MTLLoadAction,
    MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRegion, MTLResourceOptions, MTLSize,
    MTLStoreAction, MTLTextureType, MTLTextureUsage, MetalDrawableRef, RenderPassDescriptor,
    RenderPipelineDescriptor, RenderPipelineState, Texture, TextureDescriptor,
};
use zeno_core::{Color, Rect, ZenoError};
use zeno_graphics::{DrawCommand, Scene, Shape};

const SHADERS: &str = r#"
    #include <metal_stdlib>
    using namespace metal;

    struct ColorVertex {
        float2 clip_position;
        float2 local_position;
        float2 size;
        float4 color;
        float radius;
    };

    struct ColorOut {
        float4 position [[position]];
        float2 local_position;
        float2 size;
        float4 color;
        float radius;
    };

    vertex ColorOut color_vertex(uint vid [[vertex_id]], const device ColorVertex* vertices [[buffer(0)]]) {
        ColorVertex v = vertices[vid];
        ColorOut out;
        out.position = float4(v.clip_position, 0.0, 1.0);
        out.local_position = v.local_position;
        out.size = v.size;
        out.color = v.color;
        out.radius = v.radius;
        return out;
    }

    fragment float4 color_fragment(ColorOut in [[stage_in]]) {
        float radius = min(in.radius, min(in.size.x, in.size.y) * 0.5);
        if (radius > 0.0) {
            float2 nearest = clamp(in.local_position, float2(radius, radius), in.size - float2(radius, radius));
            float2 delta = in.local_position - nearest;
            if (dot(delta, delta) > radius * radius) {
                discard_fragment();
            }
        }
        return in.color;
    }

    struct TextVertex {
        float2 clip_position;
        float2 uv;
        float4 color;
    };

    struct TextOut {
        float4 position [[position]];
        float2 uv;
        float4 color;
    };

    vertex TextOut text_vertex(uint vid [[vertex_id]], const device TextVertex* vertices [[buffer(0)]]) {
        TextVertex v = vertices[vid];
        TextOut out;
        out.position = float4(v.clip_position, 0.0, 1.0);
        out.uv = v.uv;
        out.color = v.color;
        return out;
    }

    constexpr sampler text_sampler(address::clamp_to_edge, filter::linear);

    fragment float4 text_fragment(TextOut in [[stage_in]], texture2d<float> mask [[texture(0)]]) {
        float alpha = mask.sample(text_sampler, in.uv).r;
        return float4(in.color.rgb, in.color.a * alpha);
    }
"#;

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
            .map_err(|error| ZenoError::InvalidConfiguration(error.to_string()))?;

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
            .ok_or_else(|| ZenoError::InvalidConfiguration("missing metal color attachment".to_string()))?;
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
        .map_err(|error| ZenoError::InvalidConfiguration(error.to_string()))?;
    let fragment = library
        .get_function("color_fragment", None)
        .map_err(|error| ZenoError::InvalidConfiguration(error.to_string()))?;
    descriptor.set_vertex_function(Some(&vertex));
    descriptor.set_fragment_function(Some(&fragment));
    let attachment = descriptor
        .color_attachments()
        .object_at(0)
        .ok_or_else(|| ZenoError::InvalidConfiguration("missing color attachment".to_string()))?;
    attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    attachment.set_blending_enabled(true);
    attachment.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    attachment.set_source_alpha_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    device
        .new_render_pipeline_state(&descriptor)
        .map_err(|error| ZenoError::InvalidConfiguration(error.to_string()))
}

fn create_text_pipeline(
    device: &Device,
    library: &metal::Library,
) -> Result<RenderPipelineState, ZenoError> {
    let descriptor = RenderPipelineDescriptor::new();
    let vertex = library
        .get_function("text_vertex", None)
        .map_err(|error| ZenoError::InvalidConfiguration(error.to_string()))?;
    let fragment = library
        .get_function("text_fragment", None)
        .map_err(|error| ZenoError::InvalidConfiguration(error.to_string()))?;
    descriptor.set_vertex_function(Some(&vertex));
    descriptor.set_fragment_function(Some(&fragment));
    let attachment = descriptor
        .color_attachments()
        .object_at(0)
        .ok_or_else(|| ZenoError::InvalidConfiguration("missing color attachment".to_string()))?;
    attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    attachment.set_blending_enabled(true);
    attachment.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    attachment.set_source_alpha_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    device
        .new_render_pipeline_state(&descriptor)
        .map_err(|error| ZenoError::InvalidConfiguration(error.to_string()))
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

fn rasterize_text(font: &Font, text: &str, font_size: f32) -> Option<(Vec<u8>, u32, u32, f32)> {
    let size = font_size.max(12.0);
    let line_metrics = font.horizontal_line_metrics(size)?;
    let mut glyphs = Vec::new();
    let mut total_width = 0.0f32;
    let mut max_height = 0usize;

    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);
        total_width += metrics.advance_width.max(1.0);
        max_height = max_height.max(metrics.height.max(line_metrics.new_line_size.ceil() as usize));
        glyphs.push((metrics, bitmap));
    }

    let width = total_width.ceil().max(1.0) as usize;
    let height = max_height.max(1);
    let mut alpha = vec![0u8; width * height];
    let baseline = line_metrics.ascent.ceil() as isize;
    let mut pen_x = 0.0f32;

    for (metrics, bitmap) in glyphs {
        let glyph_x = (pen_x + metrics.xmin as f32).max(0.0) as usize;
        let glyph_y = (baseline - metrics.height as isize - metrics.ymin as isize).max(0) as usize;
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let src = bitmap[row * metrics.width + col];
                if src == 0 {
                    continue;
                }
                let x = glyph_x + col;
                let y = glyph_y + row;
                if x < width && y < height {
                    alpha[y * width + x] = alpha[y * width + x].max(src);
                }
            }
        }
        pen_x += metrics.advance_width;
    }

    Some((alpha, width as u32, height as u32, baseline as f32))
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

fn load_system_font() -> Option<Font> {
    let families = [
        "PingFang SC",
        "Helvetica Neue",
        "Arial",
        "Noto Sans CJK SC",
        "Noto Sans",
    ];
    for family in families {
        if let Ok(handle) = SystemSource::new().select_family_by_name(family)
            && let Some(font_handle) = handle.fonts().first()
            && let Ok(font) = font_handle.load()
            && let Some(bytes) = font.copy_font_data()
            && let Ok(parsed) = Font::from_bytes(bytes.as_slice(), fontdue::FontSettings::default())
        {
            return Some(parsed);
        }
    }
    None
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
