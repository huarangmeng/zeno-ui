use zeno_core::{
    Backend, BackendUnavailableReason, Platform, Size, WindowConfig, ZenoError, ZenoErrorCode,
};
use zeno_runtime::ResolvedSession;

use crate::{
    platform,
    shell::{NativeSurface, PlatformDescriptor},
};

use super::sessions::MobileRenderSession;
use super::shells::MobileShell;
use super::types::{
    MobileAttachContext, MobileAttachedSession, MobileHostKind, MobilePlatform,
    MobilePresenterAttachment, MobilePresenterInterface, MobilePresenterKind, MobileSessionBinding,
    MobileViewport,
};

pub(crate) fn mobile_session_error(
    code: ZenoErrorCode,
    operation: &'static str,
    message: impl Into<String>,
) -> ZenoError {
    ZenoError::invalid_configuration(code, "shell.mobile", operation, message)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MobileSessionPlan {
    platform: MobilePlatform,
    backend: Backend,
    presenter: MobilePresenterKind,
}

impl MobileSessionPlan {
    pub(crate) fn from_resolved(
        platform: MobilePlatform,
        session: &ResolvedSession,
    ) -> Result<Self, ZenoError> {
        if session.platform != platform.as_platform() {
            return Err(ZenoError::invalid_configuration(
                ZenoErrorCode::MobileSessionPlatformMismatch,
                "shell.mobile",
                "plan_session",
                format!(
                    "resolved session platform {} does not match mobile shell platform {}",
                    session.platform,
                    platform.as_platform()
                ),
            ));
        }

        let presenter = match session.backend.backend_kind {
            Backend::Skia => MobilePresenterKind::SkiaSurface,
            Backend::Impeller => MobilePresenterKind::ImpellerSurface,
        };

        Ok(Self {
            platform,
            backend: session.backend.backend_kind,
            presenter,
        })
    }

    pub(crate) fn from_binding(binding: &MobileSessionBinding) -> Self {
        Self {
            platform: binding.platform,
            backend: binding.backend,
            presenter: binding.presenter,
        }
    }

    pub(crate) fn bind(
        self,
        shell: &MobileShell,
        session: ResolvedSession,
        viewport: MobileViewport,
    ) -> Result<MobileSessionBinding, ZenoError> {
        validate_viewport(viewport)?;
        let mut session = session;
        session.window.size = Size::new(viewport.width, viewport.height);
        session.window.scale_factor = viewport.scale_factor;
        let surface = shell.create_mobile_surface(&session.window, Some(viewport));

        Ok(MobileSessionBinding {
            platform: self.platform,
            backend: self.backend,
            presenter: self.presenter,
            session,
            surface,
        })
    }

    pub(crate) fn attach(
        self,
        binding: MobileSessionBinding,
        context: MobileAttachContext,
    ) -> Result<MobileAttachedSession, ZenoError> {
        if context.platform() != self.platform.as_platform() {
            return Err(ZenoError::invalid_configuration(
                ZenoErrorCode::MobileAttachPlatformMismatch,
                "shell.mobile",
                "attach_session",
                format!(
                    "attach context platform {} does not match mobile session platform {}",
                    context.platform(),
                    self.platform.as_platform()
                ),
            ));
        }

        let (host_kind, interface) = match (self.platform, self.presenter, context.host_kind()) {
            (
                MobilePlatform::Android,
                MobilePresenterKind::SkiaSurface,
                MobileHostKind::AndroidNativeWindow,
            ) => (
                MobileHostKind::AndroidNativeWindow,
                MobilePresenterInterface::AndroidSkiaNativeWindow,
            ),
            (
                MobilePlatform::Android,
                MobilePresenterKind::ImpellerSurface,
                MobileHostKind::AndroidNativeWindow,
            ) => (
                MobileHostKind::AndroidNativeWindow,
                MobilePresenterInterface::AndroidImpellerNativeWindow,
            ),
            (MobilePlatform::Ios, MobilePresenterKind::SkiaSurface, MobileHostKind::IosView) => (
                MobileHostKind::IosView,
                MobilePresenterInterface::IosSkiaView,
            ),
            (
                MobilePlatform::Ios,
                MobilePresenterKind::SkiaSurface,
                MobileHostKind::IosMetalLayer,
            ) => (
                MobileHostKind::IosMetalLayer,
                MobilePresenterInterface::IosSkiaMetalLayer,
            ),
            (
                MobilePlatform::Ios,
                MobilePresenterKind::ImpellerSurface,
                MobileHostKind::IosMetalLayer,
            ) => (
                MobileHostKind::IosMetalLayer,
                MobilePresenterInterface::IosImpellerMetalLayer,
            ),
            _ => {
                return Err(ZenoError::BackendUnavailable {
                    backend: self.backend,
                    reason: BackendUnavailableReason::MissingPlatformSurface,
                });
            }
        };

        Ok(MobileAttachedSession {
            binding,
            attachment: MobilePresenterAttachment {
                host_kind,
                presenter: self.presenter,
                interface,
            },
            context,
        })
    }

    pub(crate) fn build(
        self,
        attached: MobileAttachedSession,
    ) -> Result<MobileRenderSession, ZenoError> {
        match attached.attachment.interface {
            MobilePresenterInterface::AndroidSkiaNativeWindow
            | MobilePresenterInterface::AndroidImpellerNativeWindow => {
                Ok(MobileRenderSession::Android(
                    super::sessions::AndroidNativeWindowSession::new(attached)?,
                ))
            }
            MobilePresenterInterface::IosSkiaView => Ok(MobileRenderSession::IosView(
                super::sessions::IosViewSession::new(attached)?,
            )),
            MobilePresenterInterface::IosSkiaMetalLayer
            | MobilePresenterInterface::IosImpellerMetalLayer => {
                Ok(MobileRenderSession::IosMetalLayer(
                    super::sessions::IosMetalLayerSession::new(attached)?,
                ))
            }
        }
    }
}

pub(crate) fn descriptor_for(platform: MobilePlatform) -> PlatformDescriptor {
    match platform {
        MobilePlatform::Android => platform::android::descriptor(),
        MobilePlatform::Ios => platform::ios::descriptor(),
    }
}

pub(crate) fn validate_viewport(viewport: MobileViewport) -> Result<(), ZenoError> {
    if viewport.width <= 0.0 || viewport.height <= 0.0 {
        return Err(ZenoError::invalid_configuration(
            ZenoErrorCode::MobileViewportInvalid,
            "shell.mobile",
            "bind_session",
            "mobile viewport must be positive",
        ));
    }
    Ok(())
}

pub(crate) fn create_mobile_surface(
    platform: MobilePlatform,
    config: &WindowConfig,
    viewport: Option<MobileViewport>,
) -> NativeSurface {
    let descriptor = descriptor_for(platform);
    let (width, height, scale_factor) = viewport
        .map(|viewport| (viewport.width, viewport.height, viewport.scale_factor))
        .unwrap_or((config.size.width, config.size.height, config.scale_factor));
    NativeSurface {
        surface: zeno_graphics::RenderSurface {
            id: format!(
                "{}-surface",
                match platform {
                    MobilePlatform::Android => Platform::Android,
                    MobilePlatform::Ios => Platform::Ios,
                }
            ),
            platform: match platform {
                MobilePlatform::Android => Platform::Android,
                MobilePlatform::Ios => Platform::Ios,
            },
            size: Size::new(width, height),
            scale_factor,
        },
        descriptor,
    }
}
