#[cfg(feature = "desktop_winit")]
use winit::event_loop::ActiveEventLoop;
use zeno_core::{Backend, BackendUnavailableReason, Platform, WindowConfig, ZenoError};

use super::DesktopRenderSession;
#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
use super::impeller_metal::ImpellerMetalSession;
#[cfg(feature = "desktop_winit")]
use super::skia_gl::SkiaGlSession;
use zeno_runtime::ResolvedSession;

#[cfg(feature = "desktop_winit")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DesktopPresenterKind {
    SkiaGl,
    #[cfg(target_os = "macos")]
    ImpellerMetal,
}

#[cfg(feature = "desktop_winit")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DesktopSessionPlan {
    pub(super) backend: Backend,
    pub(super) presenter: DesktopPresenterKind,
}

#[cfg(feature = "desktop_winit")]
impl DesktopSessionPlan {
    pub(super) fn from_resolved(resolved: &ResolvedSession) -> Result<Self, ZenoError> {
        match resolved.backend.backend_kind {
            Backend::Skia => Ok(Self {
                backend: Backend::Skia,
                presenter: DesktopPresenterKind::SkiaGl,
            }),
            Backend::Impeller if resolved.platform == Platform::MacOs => Ok(Self {
                backend: Backend::Impeller,
                #[cfg(target_os = "macos")]
                presenter: DesktopPresenterKind::ImpellerMetal,
            }),
            Backend::Impeller => Err(ZenoError::BackendUnavailable {
                backend: Backend::Impeller,
                reason: BackendUnavailableReason::NotImplementedForPlatform,
            }),
        }
    }

    pub(super) fn build(
        self,
        event_loop: &ActiveEventLoop,
        config: &WindowConfig,
    ) -> Result<DesktopRenderSession, String> {
        match self.presenter {
            DesktopPresenterKind::SkiaGl => {
                SkiaGlSession::new(event_loop, config).map(DesktopRenderSession::Skia)
            }
            #[cfg(target_os = "macos")]
            DesktopPresenterKind::ImpellerMetal => {
                ImpellerMetalSession::new(event_loop, config).map(DesktopRenderSession::Impeller)
            }
        }
    }
}

#[cfg(all(test, feature = "desktop_winit"))]
mod tests {
    use super::{DesktopPresenterKind, DesktopSessionPlan};
    use zeno_core::{Backend, Platform, WindowConfig};
    use zeno_runtime::{BackendAttempt, ResolvedBackend, ResolvedSession};

    fn resolved_session(platform: Platform, backend: Backend) -> ResolvedSession {
        ResolvedSession::new(
            platform,
            WindowConfig::default(),
            ResolvedBackend {
                backend_kind: backend,
                attempts: vec![BackendAttempt {
                    backend,
                    reason: None,
                }],
            },
            false,
        )
    }

    #[test]
    fn skia_plan_is_available_on_desktop_platforms() {
        let plan =
            DesktopSessionPlan::from_resolved(&resolved_session(Platform::Linux, Backend::Skia))
                .expect("skia desktop plan");

        assert_eq!(plan.backend, Backend::Skia);
        assert_eq!(plan.presenter, DesktopPresenterKind::SkiaGl);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_impeller_plan_uses_metal_presenter() {
        let plan = DesktopSessionPlan::from_resolved(&resolved_session(
            Platform::MacOs,
            Backend::Impeller,
        ))
        .expect("macos impeller plan");

        assert_eq!(plan.backend, Backend::Impeller);
        assert_eq!(plan.presenter, DesktopPresenterKind::ImpellerMetal);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_impeller_plan_is_rejected() {
        let error = DesktopSessionPlan::from_resolved(&resolved_session(
            Platform::Linux,
            Backend::Impeller,
        ))
        .expect_err("impeller desktop plan should fail outside macos");

        assert_eq!(
            error.error_code().as_str(),
            "backend.not_implemented_for_platform"
        );
    }
}
