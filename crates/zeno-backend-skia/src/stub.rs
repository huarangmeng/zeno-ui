use zeno_core::{Backend, ZenoError};
use zeno_graphics::{FrameReport, RenderCapabilities, RenderSurface, Renderer, Scene};

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
