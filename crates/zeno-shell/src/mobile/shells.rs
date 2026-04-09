use zeno_core::{AppConfig, Platform, WindowConfig, ZenoError};
use zeno_runtime::ResolvedSession;

use crate::shell::{NativeSurface, PlatformDescriptor, Shell};

use super::plan::{MobileSessionPlan, create_mobile_surface, descriptor_for};
use super::sessions::{BoxedMobileRenderSession, create_mobile_render_session};
use super::types::{
    MobileAttachContext, MobileAttachedSession, MobilePlatform, MobileSessionBinding,
    MobileViewport,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MobileShell {
    pub(crate) platform: MobilePlatform,
}

#[cfg(feature = "mobile_android")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AndroidShell;

#[cfg(feature = "mobile_ios")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IosShell;

impl MobileShell {
    #[cfg(feature = "mobile_android")]
    #[must_use]
    pub const fn android() -> Self {
        Self {
            platform: MobilePlatform::Android,
        }
    }

    #[cfg(feature = "mobile_ios")]
    #[must_use]
    pub const fn ios() -> Self {
        Self {
            platform: MobilePlatform::Ios,
        }
    }

    #[must_use]
    pub const fn platform(&self) -> MobilePlatform {
        self.platform
    }

    #[must_use]
    pub const fn platform_kind(&self) -> Platform {
        self.platform.as_platform()
    }

    #[must_use]
    pub fn platform_descriptor(&self) -> PlatformDescriptor {
        descriptor_for(self.platform)
    }

    #[must_use]
    pub fn create_mobile_surface(
        &self,
        config: &WindowConfig,
        viewport: Option<MobileViewport>,
    ) -> NativeSurface {
        create_mobile_surface(self.platform, config, viewport)
    }

    pub fn bind_session(
        &self,
        session: ResolvedSession,
        viewport: MobileViewport,
    ) -> Result<MobileSessionBinding, ZenoError> {
        MobileSessionPlan::from_resolved(self.platform, &session)?.bind(self, session, viewport)
    }

    pub fn prepare_app_session(
        &self,
        app_config: &AppConfig,
        viewport: MobileViewport,
    ) -> Result<MobileSessionBinding, ZenoError> {
        let session = ResolvedSession::resolve(self.platform_kind(), app_config)?;
        self.bind_session(session, viewport)
    }

    pub fn attach_session(
        &self,
        binding: MobileSessionBinding,
        context: MobileAttachContext,
    ) -> Result<MobileAttachedSession, ZenoError> {
        MobileSessionPlan::from_binding(&binding).attach(binding, context)
    }

    pub fn prepare_attached_app_session(
        &self,
        app_config: &AppConfig,
        viewport: MobileViewport,
        context: MobileAttachContext,
    ) -> Result<MobileAttachedSession, ZenoError> {
        let binding = self.prepare_app_session(app_config, viewport)?;
        self.attach_session(binding, context)
    }

    pub fn create_render_session(
        &self,
        attached: MobileAttachedSession,
    ) -> Result<BoxedMobileRenderSession, ZenoError> {
        create_mobile_render_session(attached)
    }

    pub fn prepare_render_session(
        &self,
        app_config: &AppConfig,
        viewport: MobileViewport,
        context: MobileAttachContext,
    ) -> Result<BoxedMobileRenderSession, ZenoError> {
        let attached = self.prepare_attached_app_session(app_config, viewport, context)?;
        self.create_render_session(attached)
    }
}

#[cfg(feature = "mobile_android")]
impl AndroidShell {
    #[must_use]
    pub const fn mobile() -> MobileShell {
        MobileShell::android()
    }

    pub fn bind_session(
        &self,
        session: ResolvedSession,
        viewport: MobileViewport,
    ) -> Result<MobileSessionBinding, ZenoError> {
        Self::mobile().bind_session(session, viewport)
    }

    pub fn prepare_app_session(
        &self,
        app_config: &AppConfig,
        viewport: MobileViewport,
    ) -> Result<MobileSessionBinding, ZenoError> {
        Self::mobile().prepare_app_session(app_config, viewport)
    }

    pub fn attach_session(
        &self,
        binding: MobileSessionBinding,
        context: MobileAttachContext,
    ) -> Result<MobileAttachedSession, ZenoError> {
        Self::mobile().attach_session(binding, context)
    }

    pub fn prepare_attached_app_session(
        &self,
        app_config: &AppConfig,
        viewport: MobileViewport,
        context: MobileAttachContext,
    ) -> Result<MobileAttachedSession, ZenoError> {
        Self::mobile().prepare_attached_app_session(app_config, viewport, context)
    }

    pub fn create_render_session(
        &self,
        attached: MobileAttachedSession,
    ) -> Result<BoxedMobileRenderSession, ZenoError> {
        Self::mobile().create_render_session(attached)
    }

    pub fn prepare_render_session(
        &self,
        app_config: &AppConfig,
        viewport: MobileViewport,
        context: MobileAttachContext,
    ) -> Result<BoxedMobileRenderSession, ZenoError> {
        Self::mobile().prepare_render_session(app_config, viewport, context)
    }
}

#[cfg(feature = "mobile_android")]
impl Shell for AndroidShell {
    fn create_surface(&self, config: &WindowConfig) -> NativeSurface {
        Self::mobile().create_mobile_surface(config, None)
    }
}

#[cfg(feature = "mobile_ios")]
impl IosShell {
    #[must_use]
    pub const fn mobile() -> MobileShell {
        MobileShell::ios()
    }

    pub fn bind_session(
        &self,
        session: ResolvedSession,
        viewport: MobileViewport,
    ) -> Result<MobileSessionBinding, ZenoError> {
        Self::mobile().bind_session(session, viewport)
    }

    pub fn prepare_app_session(
        &self,
        app_config: &AppConfig,
        viewport: MobileViewport,
    ) -> Result<MobileSessionBinding, ZenoError> {
        Self::mobile().prepare_app_session(app_config, viewport)
    }

    pub fn attach_session(
        &self,
        binding: MobileSessionBinding,
        context: MobileAttachContext,
    ) -> Result<MobileAttachedSession, ZenoError> {
        Self::mobile().attach_session(binding, context)
    }

    pub fn prepare_attached_app_session(
        &self,
        app_config: &AppConfig,
        viewport: MobileViewport,
        context: MobileAttachContext,
    ) -> Result<MobileAttachedSession, ZenoError> {
        Self::mobile().prepare_attached_app_session(app_config, viewport, context)
    }

    pub fn create_render_session(
        &self,
        attached: MobileAttachedSession,
    ) -> Result<BoxedMobileRenderSession, ZenoError> {
        Self::mobile().create_render_session(attached)
    }

    pub fn prepare_render_session(
        &self,
        app_config: &AppConfig,
        viewport: MobileViewport,
        context: MobileAttachContext,
    ) -> Result<BoxedMobileRenderSession, ZenoError> {
        Self::mobile().prepare_render_session(app_config, viewport, context)
    }
}

#[cfg(feature = "mobile_ios")]
impl Shell for IosShell {
    fn create_surface(&self, config: &WindowConfig) -> NativeSurface {
        Self::mobile().create_mobile_surface(config, None)
    }
}
