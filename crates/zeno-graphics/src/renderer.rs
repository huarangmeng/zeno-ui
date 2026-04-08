use zeno_core::{Backend, Platform, ZenoError};

use crate::{BackendProbe, FrameReport, RenderCapabilities, RenderSurface, Scene};

pub trait Renderer: Send + Sync {
    fn kind(&self) -> Backend;

    fn capabilities(&self) -> RenderCapabilities;

    fn render(&self, surface: &RenderSurface, scene: &Scene) -> Result<FrameReport, ZenoError>;
}

pub trait GraphicsBackend: Send + Sync {
    fn kind(&self) -> Backend;

    fn name(&self) -> &'static str;

    fn probe(&self, platform: Platform) -> BackendProbe;

    fn create_renderer(&self) -> Result<Box<dyn Renderer>, ZenoError>;
}
