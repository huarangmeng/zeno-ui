use zeno_core::{BackendKind, BackendUnavailableReason, PlatformKind, ZenoError};
use zeno_graphics::{
    BackendProbe, FrameReport, GraphicsBackend, RenderCapabilities, RenderSurface, Renderer, Scene,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct ImpellerBackend;

impl GraphicsBackend for ImpellerBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Impeller
    }

    fn name(&self) -> &'static str {
        "impeller"
    }

    fn probe(&self, platform: PlatformKind) -> BackendProbe {
        match platform {
            PlatformKind::Android | PlatformKind::IOS | PlatformKind::MacOS => {
                BackendProbe::available(
                    self.kind(),
                    RenderCapabilities {
                        gpu_compositing: true,
                        text_shaping: true,
                        filters: true,
                        offscreen_rendering: false,
                    },
                )
            }
            PlatformKind::Windows | PlatformKind::Linux => BackendProbe::unavailable(
                self.kind(),
                BackendUnavailableReason::NotImplementedForPlatform,
            ),
            PlatformKind::Unknown => BackendProbe::unavailable(
                self.kind(),
                BackendUnavailableReason::RuntimeProbeFailed("unknown target platform".to_string()),
            ),
        }
    }

    fn create_renderer(&self) -> Result<Box<dyn Renderer>, ZenoError> {
        Ok(Box::new(ImpellerRenderer))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ImpellerRenderer;

impl Renderer for ImpellerRenderer {
    fn kind(&self) -> BackendKind {
        BackendKind::Impeller
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
