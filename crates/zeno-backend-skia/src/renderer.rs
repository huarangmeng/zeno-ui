use skia_safe as sk;
use zeno_core::{Backend, ZenoError, ZenoErrorCode};
use zeno_scene::{DisplayList, FrameReport, RenderCapabilities, RenderSurface, Renderer, RetainedScene};

use crate::canvas::{SkiaTextCache, render_display_list_to_canvas, render_retained_scene_to_canvas};

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
            display_list_submit: true,
        }
    }

    fn render_retained(
        &self,
        _surface: &RenderSurface,
        scene: &mut RetainedScene,
    ) -> Result<FrameReport, ZenoError> {
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
        render_retained_scene_to_canvas(canvas, scene, &mut text_cache);

        Ok(FrameReport {
            backend: self.kind(),
            command_count: scene.packet_count(),
            resource_count: scene.resource_key_count(),
            block_count: scene.live_object_count(),
            display_item_count: 0,
            stacking_context_count: 0,
            patch_upserts: 0,
            patch_removes: 0,
            surface_id: "skia-raster".to_string(),
        })
    }

    fn render_display_list(
        &self,
        _surface: &RenderSurface,
        display_list: &DisplayList,
    ) -> Result<FrameReport, ZenoError> {
        let mut surface = sk::surfaces::raster_n32_premul((
            display_list.viewport.width as i32,
            display_list.viewport.height as i32,
        ))
        .ok_or_else(|| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendSkiaSurfaceCreateFailed,
                "backend.skia",
                "create_surface",
                "failed to create skia surface",
            )
        })?;
        let mut text_cache = SkiaTextCache::default();
        render_display_list_to_canvas(surface.canvas(), display_list, &mut text_cache);
        Ok(FrameReport {
            backend: self.kind(),
            command_count: display_list.items.len(),
            resource_count: 0,
            block_count: 0,
            display_item_count: display_list.items.len(),
            stacking_context_count: display_list.stacking_contexts.len(),
            patch_upserts: 0,
            patch_removes: 0,
            surface_id: "skia-raster".to_string(),
        })
    }
}
