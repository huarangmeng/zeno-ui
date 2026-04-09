use zeno_core::{Backend, ZenoError};
use zeno_graphics::{FrameReport, RenderCapabilities, RenderSurface, Renderer, Scene};

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
        }
    }

    fn render(&self, surface: &RenderSurface, scene: &Scene) -> Result<FrameReport, ZenoError> {
        Ok(FrameReport {
            backend: self.kind(),
            command_count: scene.commands.len(),
            resource_count: scene.resource_keys().len(),
            block_count: scene.blocks.len(),
            patch_upserts: 0,
            patch_removes: 0,
            surface_id: surface.id.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ImpellerRenderer;
    use zeno_graphics::Renderer;

    #[test]
    fn capabilities_report_offscreen_and_filters() {
        let capabilities = ImpellerRenderer.capabilities();

        assert!(capabilities.offscreen_rendering);
        assert!(capabilities.filters);
    }
}
