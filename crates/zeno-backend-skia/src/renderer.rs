use skia_safe as sk;
use zeno_core::{Backend, ZenoError};
use zeno_graphics::{FrameReport, RenderCapabilities, RenderSurface, Renderer, Scene};

use crate::canvas::render_scene_to_canvas;

#[derive(Debug, Default, Clone, Copy)]
pub struct SkiaRenderer;

impl Renderer for SkiaRenderer {
    fn kind(&self) -> Backend {
        Backend::Skia
    }

    fn capabilities(&self) -> RenderCapabilities {
        RenderCapabilities {
            gpu_compositing: false,
            text_shaping: true,
            filters: true,
            offscreen_rendering: true,
        }
    }

    fn render(&self, _surface: &RenderSurface, scene: &Scene) -> Result<FrameReport, ZenoError> {
        let mut surface = sk::surfaces::raster_n32_premul((
            scene.size.width as i32,
            scene.size.height as i32,
        ))
        .ok_or_else(|| ZenoError::InvalidConfiguration("failed to create skia surface".into()))?;
        let canvas = surface.canvas();
        render_scene_to_canvas(canvas, scene);

        let image = surface.image_snapshot();
        if let Some(data) = image.encode(None, sk::EncodedImageFormat::PNG, 100) {
            let _ = std::fs::create_dir_all("target");
            let _ = std::fs::write("target/zeno_skia_output.png", data.as_bytes().to_vec());
        }

        Ok(FrameReport {
            backend: self.kind(),
            command_count: scene.commands.len(),
            surface_id: "skia-raster".to_string(),
        })
    }
}
