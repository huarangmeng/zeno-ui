use zeno_core::{Backend, Platform, ZenoError, ZenoErrorCode};

use crate::{BackendProbe, FrameReport, RenderCapabilities, RenderSurface, Scene, SceneSubmit};

pub trait Renderer: Send + Sync {
    fn kind(&self) -> Backend;

    fn capabilities(&self) -> RenderCapabilities;

    fn render(&self, surface: &RenderSurface, scene: &Scene) -> Result<FrameReport, ZenoError>;

    fn submit(
        &self,
        surface: &RenderSurface,
        submit: &SceneSubmit,
    ) -> Result<FrameReport, ZenoError> {
        let scene = submit.snapshot(None).ok_or_else(|| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::GraphicsScenePatchWithoutBase,
                "graphics.renderer",
                "submit",
                "scene patch requires a previous snapshot",
            )
        })?;
        self.render(surface, &scene)
    }
}

pub trait RenderSession {
    fn kind(&self) -> Backend;

    fn capabilities(&self) -> RenderCapabilities;

    fn surface(&self) -> &RenderSurface;

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError>;

    fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError>;
}

pub trait GraphicsBackend: Send + Sync {
    fn kind(&self) -> Backend;

    fn name(&self) -> &'static str;

    fn probe(&self, platform: Platform) -> BackendProbe;

    fn create_renderer(&self) -> Result<Box<dyn Renderer>, ZenoError>;
}
