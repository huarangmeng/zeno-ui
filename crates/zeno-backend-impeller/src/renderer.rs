use zeno_core::{Backend, ZenoError};
use zeno_scene::{DisplayList, FrameReport, RenderCapabilities, RenderSurface, Renderer, TileGrid};

#[derive(Debug, Default, Clone, Copy)]
pub struct ImpellerRenderer;

impl Renderer for ImpellerRenderer {
    fn kind(&self) -> Backend {
        Backend::Impeller
    }

    fn capabilities(&self) -> RenderCapabilities {
        RenderCapabilities {
            gpu_compositing: true,
            text_shaping: true,
            filters: true,
            offscreen_rendering: true,
            display_list_submit: true,
        }
    }

    fn render_display_list(
        &self,
        surface: &RenderSurface,
        display_list: &DisplayList,
    ) -> Result<FrameReport, ZenoError> {
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
            surface_id: surface.id.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ImpellerRenderer;
    use zeno_scene::Renderer;

    #[test]
    fn capabilities_report_offscreen_and_filters() {
        let capabilities = ImpellerRenderer.capabilities();

        assert!(capabilities.offscreen_rendering);
        assert!(capabilities.filters);
        assert!(capabilities.display_list_submit);
    }
}
