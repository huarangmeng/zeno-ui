use zeno_core::{Backend, Platform, ZenoError};

use crate::{BackendProbe, DisplayList, FrameReport, RenderCapabilities, RenderSurface};

pub trait Renderer: Send + Sync {
    fn kind(&self) -> Backend;

    fn capabilities(&self) -> RenderCapabilities;

    fn render_display_list(
        &self,
        surface: &RenderSurface,
        display_list: &DisplayList,
    ) -> Result<FrameReport, ZenoError>;
}

pub trait RenderSession {
    fn kind(&self) -> Backend;

    fn capabilities(&self) -> RenderCapabilities;

    fn surface(&self) -> &RenderSurface;

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError>;

    fn submit_display_list(
        &mut self,
        display_list: &DisplayList,
        dirty_bounds: Option<zeno_core::Rect>,
        patch_upserts: usize,
        patch_removes: usize,
    ) -> Result<FrameReport, ZenoError>;
}

pub trait GraphicsBackend: Send + Sync {
    fn kind(&self) -> Backend;

    fn name(&self) -> &'static str;

    fn probe(&self, platform: Platform) -> BackendProbe;

    fn create_renderer(&self) -> Result<Box<dyn Renderer>, ZenoError>;
}
