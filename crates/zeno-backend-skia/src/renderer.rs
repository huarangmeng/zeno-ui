use skia_safe as sk;
use zeno_core::{Backend, ZenoError, ZenoErrorCode};
use zeno_scene::{FrameReport, RenderCapabilities, RenderSurface, Renderer, Scene};

use crate::canvas::{SkiaTextCache, render_scene_to_canvas};

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
        let mut text_cache = SkiaTextCache::default();
        let mut surface =
            sk::surfaces::raster_n32_premul((scene.size.width as i32, scene.size.height as i32))
                .ok_or_else(|| {
                    ZenoError::invalid_configuration(
                        ZenoErrorCode::BackendSkiaSurfaceCreateFailed,
                        "backend.skia",
                        "create_surface",
                        "failed to create skia surface",
                    )
                })?;
        let canvas = surface.canvas();
        render_scene_to_canvas(canvas, scene, &mut text_cache);

        Ok(FrameReport {
            backend: self.kind(),
            command_count: scene.packet_count(),
            resource_count: scene.resource_keys().len(),
            block_count: scene.objects.len(),
            patch_upserts: 0,
            patch_removes: 0,
            surface_id: "skia-raster".to_string(),
        })
    }
}
