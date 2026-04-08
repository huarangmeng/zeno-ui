use zeno_core::{Backend, BackendUnavailableReason, Platform, ZenoError};
use zeno_graphics::{BackendProbe, GraphicsBackend, RenderCapabilities, Renderer};

use crate::renderer::ImpellerRenderer;

#[derive(Debug, Default, Clone, Copy)]
pub struct ImpellerBackend;

impl GraphicsBackend for ImpellerBackend {
    fn kind(&self) -> Backend {
        Backend::Impeller
    }

    fn name(&self) -> &'static str {
        "impeller"
    }

    fn probe(&self, platform: Platform) -> BackendProbe {
        match platform {
            Platform::MacOs => BackendProbe::available(self.kind(), RenderCapabilities::minimal()),
            Platform::Android
            | Platform::Ios
            | Platform::Windows
            | Platform::Linux => BackendProbe::unavailable(
                self.kind(),
                BackendUnavailableReason::NotImplementedForPlatform,
            ),
            Platform::Unknown => BackendProbe::unavailable(
                self.kind(),
                BackendUnavailableReason::RuntimeProbeFailed("unknown target platform".to_string()),
            ),
        }
    }

    fn create_renderer(&self) -> Result<Box<dyn Renderer>, ZenoError> {
        Ok(Box::new(ImpellerRenderer))
    }
}
