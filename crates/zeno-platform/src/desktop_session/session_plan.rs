#[cfg(feature = "desktop_winit")]
use winit::event_loop::ActiveEventLoop;
use zeno_core::{Backend, BackendUnavailableReason, Platform, WindowConfig, ZenoError};

use super::DesktopRenderSession;
#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
use super::impeller_metal::ImpellerMetalSession;
#[cfg(feature = "desktop_winit")]
use super::skia_gl::SkiaGlSession;
use crate::session::ResolvedSession;
use crate::shell::{NativeSurface, NativeSurfaceHostRequirement, host_requirement_for_backend};

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
    pub(super) fn from_resolved(
        resolved: &ResolvedSession,
        native_surface: &NativeSurface,
    ) -> Result<Self, ZenoError> {
        let expected_host =
            host_requirement_for_backend(resolved.platform, resolved.backend.backend_kind)?;
        if native_surface.host_requirement != NativeSurfaceHostRequirement::DesktopWindow
            || expected_host != NativeSurfaceHostRequirement::DesktopWindow
        {
            return Err(ZenoError::BackendUnavailable {
                backend: resolved.backend.backend_kind,
                reason: BackendUnavailableReason::MissingPlatformSurface,
            });
        }
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
        native_surface: &NativeSurface,
        config: &WindowConfig,
    ) -> Result<DesktopRenderSession, String> {
        match self.presenter {
            DesktopPresenterKind::SkiaGl => SkiaGlSession::new(event_loop, config, native_surface)
                .map(DesktopRenderSession::Skia),
            #[cfg(target_os = "macos")]
            DesktopPresenterKind::ImpellerMetal => {
                ImpellerMetalSession::new(event_loop, config, native_surface)
                    .map(DesktopRenderSession::Impeller)
            }
        }
    }
}

#[cfg(all(test, feature = "desktop_winit"))]
mod tests {
    use super::{DesktopPresenterKind, DesktopSessionPlan};
    use crate::NativeSurfaceHostRequirement;
    use crate::session::{BackendAttempt, ResolvedBackend, ResolvedSession};
    use zeno_core::{Backend, Platform, WindowConfig};

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
        let native_surface = crate::shell::create_native_surface(
            &WindowConfig::default(),
            None,
            Some(Backend::Skia),
            NativeSurfaceHostRequirement::DesktopWindow,
        );
        let plan = DesktopSessionPlan::from_resolved(
            &resolved_session(Platform::Linux, Backend::Skia),
            &native_surface,
        )
        .expect("skia desktop plan");

        assert_eq!(plan.backend, Backend::Skia);
        assert_eq!(plan.presenter, DesktopPresenterKind::SkiaGl);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_impeller_plan_uses_metal_presenter() {
        let native_surface = crate::shell::create_native_surface(
            &WindowConfig::default(),
            None,
            Some(Backend::Impeller),
            NativeSurfaceHostRequirement::DesktopWindow,
        );
        let plan = DesktopSessionPlan::from_resolved(
            &resolved_session(Platform::MacOs, Backend::Impeller),
            &native_surface,
        )
        .expect("macos impeller plan");

        assert_eq!(plan.backend, Backend::Impeller);
        assert_eq!(plan.presenter, DesktopPresenterKind::ImpellerMetal);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_impeller_plan_is_rejected() {
        let native_surface = crate::shell::create_native_surface(
            &WindowConfig::default(),
            None,
            Some(Backend::Impeller),
            NativeSurfaceHostRequirement::DesktopWindow,
        );
        let error = DesktopSessionPlan::from_resolved(
            &resolved_session(Platform::Linux, Backend::Impeller),
            &native_surface,
        )
        .expect_err("impeller desktop plan should fail outside macos");

        assert_eq!(
            error.error_code().as_str(),
            "backend.not_implemented_for_platform"
        );
    }
}
