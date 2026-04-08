use zeno_core::{Backend, Platform, ZenoError};
use zeno_graphics::{BackendProbe, GraphicsBackend, RenderCapabilities, Renderer};

#[cfg(not(feature = "native_skia"))]
use crate::stub::StubSkiaRenderer as SelectedRenderer;
#[cfg(feature = "native_skia")]
use crate::renderer::SkiaRenderer as SelectedRenderer;

#[derive(Debug, Default, Clone, Copy)]
pub struct SkiaBackend;

impl GraphicsBackend for SkiaBackend {
    fn kind(&self) -> Backend {
        Backend::Skia
    }

    fn name(&self) -> &'static str {
        "skia"
    }

    fn probe(&self, _platform: Platform) -> BackendProbe {
        BackendProbe::available(self.kind(), RenderCapabilities::minimal())
    }

    fn create_renderer(&self) -> Result<Box<dyn Renderer>, ZenoError> {
        Ok(Box::new(SelectedRenderer))
    }
}
