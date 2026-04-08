use zeno_core::{BackendKind, Color, PlatformKind, ZenoError};
use zeno_graphics::{
    BackendProbe, DrawCommand, FrameReport, GraphicsBackend, RenderCapabilities, RenderSurface,
    Renderer, Scene, Shape,
};

#[cfg(feature = "real_skia")]
mod real {
    use super::*;
    use skia_safe as sk;

    #[derive(Debug, Default, Clone, Copy)]
    pub struct SkiaRenderer;

    impl Renderer for SkiaRenderer {
        fn kind(&self) -> BackendKind {
            BackendKind::Skia
        }

        fn capabilities(&self) -> RenderCapabilities {
            RenderCapabilities {
                gpu_compositing: false,
                text_shaping: true,
                filters: true,
                offscreen_rendering: true,
            }
        }

        fn render(&self, surface: &RenderSurface, scene: &Scene) -> Result<FrameReport, ZenoError> {
            let mut surface = sk::Surface::new_raster_n32_premul((
                scene.size.width as i32,
                scene.size.height as i32,
            ))
            .ok_or_else(|| ZenoError::InvalidConfiguration("failed to create skia surface".into()))?;
            let canvas = surface.canvas();

            for cmd in &scene.commands {
                match cmd {
                    DrawCommand::Clear(color) => {
                        canvas.clear(sk_color(*color));
                    }
                    DrawCommand::Fill { shape, brush } => {
                        let mut paint = sk::Paint::default();
                        paint.set_style(skia_safe::paint::Style::Fill);
                        if let zeno_graphics::Brush::Solid(c) = brush {
                            paint.set_color(sk_color(*c));
                        }
                        draw_shape(canvas, shape, &paint);
                    }
                    DrawCommand::Stroke { shape, stroke } => {
                        let mut paint = sk::Paint::default();
                        paint.set_style(skia_safe::paint::Style::Stroke);
                        paint.set_stroke_width(stroke.width);
                        paint.set_color(sk_color(stroke.color));
                        draw_shape(canvas, shape, &paint);
                    }
                    DrawCommand::Text { position, layout, color } => {
                        let font = sk::Font::default();
                        let mut paint = sk::Paint::default();
                        paint.set_color(sk_color(*color));
                        let p = (position.x, position.y);
                        canvas.draw_str(layout.paragraph.text.as_str(), p, &font, &paint);
                    }
                }
            }

            if let Some(image) = surface.image_snapshot() {
                if let Some(data) = image.encode(None, sk::EncodedImageFormat::PNG, 100) {
                    let _ = std::fs::create_dir_all("target");
                    let _ = std::fs::write("target/zeno_skia_output.png", data.as_bytes());
                }
            }

            Ok(FrameReport {
                backend: self.kind(),
                command_count: scene.commands.len(),
                surface_id: surface::id_string(surface),
            })
        }
    }

    fn sk_color(c: Color) -> sk::Color {
        sk::Color::from_argb(c.alpha, c.red, c.green, c.blue)
    }

    fn draw_shape(canvas: &mut sk::Canvas, shape: &Shape, paint: &sk::Paint) {
        match shape {
            Shape::Rect(r) => {
                let rect = sk::Rect::from_xywh(r.origin.x, r.origin.y, r.size.width, r.size.height);
                canvas.draw_rect(rect, paint);
            }
            Shape::RoundedRect { rect, radius } => {
                let rr = sk::RRect::new_rect_xy(
                    sk::Rect::from_xywh(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height),
                    *radius,
                    *radius,
                );
                canvas.draw_rrect(rr, paint);
            }
            Shape::Circle { center, radius } => {
                canvas.draw_circle((center.x, center.y), *radius, paint);
            }
        }
    }

    mod surface {
        pub fn id_string(_surface: skia_safe::Surface) -> String {
            "skia-raster".to_string()
        }
    }
}

#[cfg(not(feature = "real_skia"))]
mod stub {
    use super::*;

    #[derive(Debug, Default, Clone, Copy)]
    pub struct SkiaRenderer;

    impl Renderer for SkiaRenderer {
        fn kind(&self) -> BackendKind {
            BackendKind::Skia
        }

        fn capabilities(&self) -> RenderCapabilities {
            RenderCapabilities {
                gpu_compositing: true,
                text_shaping: true,
                filters: true,
                offscreen_rendering: true,
            }
        }

        fn render(&self, surface: &RenderSurface, scene: &Scene) -> Result<FrameReport, ZenoError> {
            Ok(FrameReport {
                backend: self.kind(),
                command_count: scene.commands.len(),
                surface_id: surface.id.clone(),
            })
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SkiaBackend;

impl GraphicsBackend for SkiaBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Skia
    }

    fn name(&self) -> &'static str {
        "skia"
    }

    fn probe(&self, _platform: PlatformKind) -> BackendProbe {
        BackendProbe::available(self.kind(), RenderCapabilities::minimal())
    }

    fn create_renderer(&self) -> Result<Box<dyn Renderer>, ZenoError> {
        #[cfg(feature = "real_skia")]
        {
            Ok(Box::new(real::SkiaRenderer))
        }
        #[cfg(not(feature = "real_skia"))]
        {
            Ok(Box::new(stub::SkiaRenderer))
        }
    }
}
