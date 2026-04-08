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
            offscreen_rendering: false,
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
