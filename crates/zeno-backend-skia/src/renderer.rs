use skia_safe as sk;
use zeno_core::{Backend, ZenoError, ZenoErrorCode};
use zeno_scene::{DisplayList, FrameReport, RenderCapabilities, RenderSurface, Renderer, TileGrid};

use crate::canvas::{SkiaTextCache, render_display_list_to_canvas};

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
            damage_rect_count: 0,
            damage_full: true,
            dirty_tile_count: TileGrid::for_viewport(display_list.viewport).tile_count(),
            cached_tile_count: 0,
            reraster_tile_count: TileGrid::for_viewport(display_list.viewport).tile_count(),
            raster_batch_tile_count: TileGrid::for_viewport(display_list.viewport).tile_count(),
            composite_tile_count: TileGrid::for_viewport(display_list.viewport).tile_count(),
            compositor_layer_count: display_list.stacking_contexts.len() + 1,
            offscreen_layer_count: display_list
                .stacking_contexts
                .iter()
                .filter(|context| context.needs_offscreen)
                .count(),
            tile_content_handle_count: 0,
            compositor_task_count: 0,
            compositor_queue_depth: 0,
            compositor_dropped_frame_count: 0,
            compositor_processed_frame_count: 0,
            released_tile_resource_count: 0,
            evicted_tile_resource_count: 0,
            budget_evicted_tile_resource_count: 0,
            age_evicted_tile_resource_count: 0,
            descriptor_limit_evicted_tile_resource_count: 0,
            reused_tile_resource_count: 0,
            reusable_tile_resource_count: 0,
            reusable_tile_resource_bytes: 0,
            tile_resource_reuse_budget_bytes: 0,
            compositor_worker_threaded: false,
            compositor_worker_alive: false,
            composite_executed_layer_count: 0,
            composite_executed_tile_count: 0,
            composite_offscreen_step_count: 0,
            surface_id: "skia-raster".to_string(),
        })
    }
}
