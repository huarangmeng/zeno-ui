use zeno_core::{Backend, ZenoError};
use zeno_scene::{DisplayList, FrameReport, RenderCapabilities, RenderSurface, Renderer};

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
            patch_upserts: 0,
            patch_removes: 0,
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
