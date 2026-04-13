use zeno_core::{Backend, ZenoError};
use zeno_scene::{DisplayList, FrameReport, RenderCapabilities, RenderSurface, Renderer, RetainedScene};

#[derive(Debug, Default, Clone, Copy)]
pub struct StubSkiaRenderer;

impl Renderer for StubSkiaRenderer {
    fn kind(&self) -> Backend {
        Backend::Skia
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

    fn render_retained(
        &self,
        surface: &RenderSurface,
        scene: &mut RetainedScene,
    ) -> Result<FrameReport, ZenoError> {
        Ok(FrameReport {
            backend: self.kind(),
            command_count: scene.packet_count(),
            resource_count: scene.resource_key_count(),
            block_count: scene.live_object_count(),
            display_item_count: 0,
            stacking_context_count: 0,
            patch_upserts: 0,
            patch_removes: 0,
            surface_id: surface.id.clone(),
        })
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
            patch_upserts: 0,
            patch_removes: 0,
            surface_id: surface.id.clone(),
        })
    }
}
