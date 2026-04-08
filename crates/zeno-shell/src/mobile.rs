use std::num::NonZeroUsize;

use zeno_core::{
    AppConfig, Backend, BackendUnavailableReason, Platform, Size, WindowConfig, ZenoError,
    ZenoErrorCode,
};
use zeno_graphics::{FrameReport, RenderCapabilities, RenderSession, RenderSurface, Scene, SceneSubmit};
use zeno_runtime::ResolvedSession;

use crate::{
    platform,
    shell::{NativeSurface, PlatformDescriptor, Shell},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobilePlatform {
    Android,
    Ios,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MobileViewport {
    pub width: f32,
    pub height: f32,
    pub scale_factor: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MobileSessionBinding {
    pub platform: MobilePlatform,
    pub backend: Backend,
    pub presenter: MobilePresenterKind,
    pub session: ResolvedSession,
    pub surface: NativeSurface,
}

impl MobileSessionBinding {
    #[must_use]
    pub fn surface_id(&self) -> &str {
        &self.surface.surface.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobileHostKind {
    AndroidNativeWindow,
    IosView,
    IosMetalLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AndroidAttachContext {
    pub native_window: NonZeroUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IosViewAttachContext {
    pub ui_view: NonZeroUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IosMetalLayerAttachContext {
    pub metal_layer: NonZeroUsize,
    pub ui_view: Option<NonZeroUsize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobileAttachContext {
    AndroidSurface(AndroidAttachContext),
    IosView(IosViewAttachContext),
    IosMetalLayer(IosMetalLayerAttachContext),
}

impl MobileAttachContext {
    #[must_use]
    pub const fn platform(self) -> Platform {
        match self {
            Self::AndroidSurface(_) => Platform::Android,
            Self::IosView(_) | Self::IosMetalLayer(_) => Platform::Ios,
        }
    }

    #[must_use]
    pub const fn host_kind(self) -> MobileHostKind {
        match self {
            Self::AndroidSurface(_) => MobileHostKind::AndroidNativeWindow,
            Self::IosView(_) => MobileHostKind::IosView,
            Self::IosMetalLayer(_) => MobileHostKind::IosMetalLayer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MobilePresenterAttachment {
    pub host_kind: MobileHostKind,
    pub presenter: MobilePresenterKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MobileAttachedSession {
    pub binding: MobileSessionBinding,
    pub attachment: MobilePresenterAttachment,
    pub context: MobileAttachContext,
}

impl MobileAttachedSession {
    #[must_use]
    pub fn surface_id(&self) -> &str {
        self.binding.surface_id()
    }
}

pub trait MobileRenderSessionHandle: RenderSession {
    fn attachment(&self) -> &MobilePresenterAttachment;
}

pub type BoxedMobileRenderSession = Box<dyn MobileRenderSessionHandle>;

fn mobile_session_error(
    code: ZenoErrorCode,
    operation: &'static str,
    message: impl Into<String>,
) -> ZenoError {
    ZenoError::invalid_configuration(code, "shell.mobile", operation, message)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobilePresenterKind {
    SkiaSurface,
    ImpellerSurface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MobileSessionPlan {
    platform: MobilePlatform,
    backend: Backend,
    presenter: MobilePresenterKind,
}

impl MobileSessionPlan {
    fn from_resolved(
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

    fn from_binding(binding: &MobileSessionBinding) -> Self {
        Self {
            platform: binding.platform,
            backend: binding.backend,
            presenter: binding.presenter,
        }
    }

    fn bind(
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

    fn attach(
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

        let host_kind = match (self.platform, self.presenter, context.host_kind()) {
            (
                MobilePlatform::Android,
                MobilePresenterKind::SkiaSurface | MobilePresenterKind::ImpellerSurface,
                MobileHostKind::AndroidNativeWindow,
            ) => MobileHostKind::AndroidNativeWindow,
            (
                MobilePlatform::Ios,
                MobilePresenterKind::SkiaSurface,
                MobileHostKind::IosView | MobileHostKind::IosMetalLayer,
            ) => context.host_kind(),
            (
                MobilePlatform::Ios,
                MobilePresenterKind::ImpellerSurface,
                MobileHostKind::IosMetalLayer,
            ) => MobileHostKind::IosMetalLayer,
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
            },
            context,
        })
    }

    fn build(
        self,
        attached: MobileAttachedSession,
    ) -> Result<MobileRenderSession, String> {
        match attached.attachment.host_kind {
            MobileHostKind::AndroidNativeWindow => {
                Ok(MobileRenderSession::Android(AndroidNativeWindowSession::new(
                    attached,
                )))
            }
            MobileHostKind::IosView => Ok(MobileRenderSession::IosView(IosViewSession::new(attached))),
            MobileHostKind::IosMetalLayer => {
                Ok(MobileRenderSession::IosMetalLayer(IosMetalLayerSession::new(attached)))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MobileShell {
    platform: MobilePlatform,
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

fn descriptor_for(platform: MobilePlatform) -> PlatformDescriptor {
    match platform {
        MobilePlatform::Android => platform::android::descriptor(),
        MobilePlatform::Ios => platform::ios::descriptor(),
    }
}

impl MobilePlatform {
    #[must_use]
    pub const fn as_platform(self) -> Platform {
        match self {
            Self::Android => Platform::Android,
            Self::Ios => Platform::Ios,
        }
    }
}

fn validate_viewport(viewport: MobileViewport) -> Result<(), ZenoError> {
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

fn create_mobile_surface(
    platform: MobilePlatform,
    config: &WindowConfig,
    viewport: Option<MobileViewport>,
) -> NativeSurface {
    let descriptor = descriptor_for(platform);
    let (width, height, scale_factor) = viewport
        .map(|viewport| (viewport.width, viewport.height, viewport.scale_factor))
        .unwrap_or((config.size.width, config.size.height, config.scale_factor));
    NativeSurface {
        surface: RenderSurface {
            id: format!("{}-surface", match platform {
                MobilePlatform::Android => Platform::Android,
                MobilePlatform::Ios => Platform::Ios,
            }),
            platform: match platform {
                MobilePlatform::Android => Platform::Android,
                MobilePlatform::Ios => Platform::Ios,
            },
            size: zeno_core::Size::new(width, height),
            scale_factor,
        },
        descriptor,
    }
}

pub fn create_mobile_render_session(
    attached: MobileAttachedSession,
) -> Result<BoxedMobileRenderSession, ZenoError> {
    MobileSessionPlan::from_binding(&attached.binding)
        .build(attached)
        .map(|session| Box::new(session) as BoxedMobileRenderSession)
        .map_err(|error| {
            mobile_session_error(
                ZenoErrorCode::SessionCreateRenderSessionFailed,
                "create_render_session",
                error,
            )
        })
}

enum MobileRenderSession {
    Android(AndroidNativeWindowSession),
    IosView(IosViewSession),
    IosMetalLayer(IosMetalLayerSession),
}

impl MobileRenderSessionHandle for MobileRenderSession {
    fn attachment(&self) -> &MobilePresenterAttachment {
        match self {
            Self::Android(session) => &session.attachment,
            Self::IosView(session) => &session.attachment,
            Self::IosMetalLayer(session) => &session.attachment,
        }
    }
}

impl RenderSession for MobileRenderSession {
    fn kind(&self) -> Backend {
        match self {
            Self::Android(session) => session.kind(),
            Self::IosView(session) => session.kind(),
            Self::IosMetalLayer(session) => session.kind(),
        }
    }

    fn capabilities(&self) -> RenderCapabilities {
        match self {
            Self::Android(session) => session.capabilities(),
            Self::IosView(session) => session.capabilities(),
            Self::IosMetalLayer(session) => session.capabilities(),
        }
    }

    fn surface(&self) -> &RenderSurface {
        match self {
            Self::Android(session) => session.surface(),
            Self::IosView(session) => session.surface(),
            Self::IosMetalLayer(session) => session.surface(),
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError> {
        match self {
            Self::Android(session) => session.resize(width, height),
            Self::IosView(session) => session.resize(width, height),
            Self::IosMetalLayer(session) => session.resize(width, height),
        }
    }

    fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError> {
        match self {
            Self::Android(session) => session.submit_scene(submit),
            Self::IosView(session) => session.submit_scene(submit),
            Self::IosMetalLayer(session) => session.submit_scene(submit),
        }
    }
}

struct AndroidNativeWindowSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    last_scene: Option<Scene>,
}

impl AndroidNativeWindowSession {
    fn new(attached: MobileAttachedSession) -> Self {
        Self {
            backend: attached.binding.backend,
            attachment: attached.attachment,
            surface: attached.binding.surface.surface,
            last_scene: None,
        }
    }
}

struct IosViewSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    last_scene: Option<Scene>,
}

impl IosViewSession {
    fn new(attached: MobileAttachedSession) -> Self {
        Self {
            backend: attached.binding.backend,
            attachment: attached.attachment,
            surface: attached.binding.surface.surface,
            last_scene: None,
        }
    }
}

struct IosMetalLayerSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    last_scene: Option<Scene>,
}

impl IosMetalLayerSession {
    fn new(attached: MobileAttachedSession) -> Self {
        Self {
            backend: attached.binding.backend,
            attachment: attached.attachment,
            surface: attached.binding.surface.surface,
            last_scene: None,
        }
    }
}

fn mobile_capabilities_for(backend: Backend) -> RenderCapabilities {
    match backend {
        Backend::Skia | Backend::Impeller => RenderCapabilities {
            gpu_compositing: true,
            text_shaping: true,
            filters: true,
            offscreen_rendering: false,
        },
    }
}

fn resize_mobile_surface(
    surface: &mut RenderSurface,
    width: u32,
    height: u32,
) -> Result<(), ZenoError> {
    if width == 0 {
        return Err(mobile_session_error(
            ZenoErrorCode::SessionInvalidWindowWidth,
            "resize",
            "invalid mobile surface width",
        ));
    }
    if height == 0 {
        return Err(mobile_session_error(
            ZenoErrorCode::SessionInvalidWindowHeight,
            "resize",
            "invalid mobile surface height",
        ));
    }
    surface.size = Size::new(width as f32, height as f32);
    Ok(())
}

fn submit_mobile_scene(
    backend: Backend,
    surface: &RenderSurface,
    last_scene: &mut Option<Scene>,
    submit: &SceneSubmit,
) -> Result<FrameReport, ZenoError> {
    let scene = submit.snapshot(last_scene.as_ref()).ok_or_else(|| {
        mobile_session_error(
            ZenoErrorCode::GraphicsScenePatchWithoutBase,
            "submit_scene",
            "scene patch requires a previous snapshot",
        )
    })?;
    let (patch_upserts, patch_removes) = patch_stats(submit);
    let report = FrameReport {
        backend,
        command_count: scene.commands.len(),
        resource_count: scene.resource_keys().len(),
        block_count: scene.blocks.len(),
        patch_upserts,
        patch_removes,
        surface_id: surface.id.clone(),
    };
    *last_scene = Some(scene);
    Ok(report)
}

impl RenderSession for AndroidNativeWindowSession {
    fn kind(&self) -> Backend {
        self.backend
    }

    fn capabilities(&self) -> RenderCapabilities {
        mobile_capabilities_for(self.backend)
    }

    fn surface(&self) -> &RenderSurface {
        &self.surface
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError> {
        resize_mobile_surface(&mut self.surface, width, height)
    }

    fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError> {
        submit_mobile_scene(self.backend, &self.surface, &mut self.last_scene, submit)
    }
}

impl RenderSession for IosViewSession {
    fn kind(&self) -> Backend {
        self.backend
    }

    fn capabilities(&self) -> RenderCapabilities {
        mobile_capabilities_for(self.backend)
    }

    fn surface(&self) -> &RenderSurface {
        &self.surface
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError> {
        resize_mobile_surface(&mut self.surface, width, height)
    }

    fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError> {
        submit_mobile_scene(self.backend, &self.surface, &mut self.last_scene, submit)
    }
}

impl RenderSession for IosMetalLayerSession {
    fn kind(&self) -> Backend {
        self.backend
    }

    fn capabilities(&self) -> RenderCapabilities {
        mobile_capabilities_for(self.backend)
    }

    fn surface(&self) -> &RenderSurface {
        &self.surface
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError> {
        resize_mobile_surface(&mut self.surface, width, height)
    }

    fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError> {
        submit_mobile_scene(self.backend, &self.surface, &mut self.last_scene, submit)
    }
}

fn patch_stats(submit: &SceneSubmit) -> (usize, usize) {
    match submit {
        SceneSubmit::Full(scene) => (scene.blocks.len(), 0),
        SceneSubmit::Patch { patch, .. } => (patch.upserts.len(), patch.removes.len()),
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::{
        create_mobile_render_session, AndroidAttachContext, IosMetalLayerAttachContext,
        IosViewAttachContext, MobileAttachContext, MobileHostKind, MobilePlatform,
        MobilePresenterAttachment, MobilePresenterKind, MobileShell, MobileViewport,
    };
    use zeno_core::{AppConfig, Backend, Color, Platform, RendererConfig, Size, WindowConfig, ZenoErrorCode};
    use zeno_graphics::{DrawCommand, Scene, SceneSubmit};
    use zeno_runtime::{BackendAttempt, ResolvedBackend, ResolvedSession};

    fn fake_handle(seed: usize) -> NonZeroUsize {
        NonZeroUsize::new(seed).expect("non-zero handle")
    }

    fn test_submit() -> SceneSubmit {
        SceneSubmit::Full(Scene {
            size: Size::new(120.0, 80.0),
            commands: vec![DrawCommand::Clear(Color::WHITE)],
            blocks: Vec::new(),
        })
    }

    #[test]
    fn mobile_shell_uses_requested_platform_descriptor() {
        let shell = MobileShell {
            platform: MobilePlatform::Android,
        };
        let surface = shell.create_mobile_surface(&WindowConfig::default(), None);

        assert_eq!(surface.descriptor.platform, zeno_core::Platform::Android);
        assert_eq!(surface.surface.platform, zeno_core::Platform::Android);
    }

    #[test]
    fn bind_session_applies_viewport_size_and_scale() {
        let shell = MobileShell {
            platform: MobilePlatform::Ios,
        };
        let session = ResolvedSession::new(
            zeno_core::Platform::Ios,
            WindowConfig::default(),
            ResolvedBackend {
                backend_kind: Backend::Skia,
                attempts: vec![BackendAttempt {
                    backend: Backend::Skia,
                    reason: None,
                }],
            },
            false,
        );
        let binding = shell
            .bind_session(
                session,
                MobileViewport {
                    width: 390.0,
                    height: 844.0,
                    scale_factor: 3.0,
                },
            )
            .expect("mobile session binding");

        assert_eq!(binding.backend, Backend::Skia);
        assert_eq!(binding.presenter, MobilePresenterKind::SkiaSurface);
        assert_eq!(binding.session.window.size.width, 390.0);
        assert_eq!(binding.session.window.size.height, 844.0);
        assert_eq!(binding.session.window.scale_factor, 3.0);
        assert_eq!(binding.surface.surface.size.width, 390.0);
        assert_eq!(binding.surface.surface.size.height, 844.0);
        assert_eq!(binding.surface.surface.scale_factor, 3.0);
        assert_eq!(binding.surface.surface.platform, zeno_core::Platform::Ios);
    }

    #[test]
    fn bind_session_rejects_platform_mismatch() {
        let shell = MobileShell {
            platform: MobilePlatform::Android,
        };
        let session = ResolvedSession::new(
            Platform::Ios,
            WindowConfig::default(),
            ResolvedBackend {
                backend_kind: Backend::Skia,
                attempts: vec![BackendAttempt {
                    backend: Backend::Skia,
                    reason: None,
                }],
            },
            false,
        );
        let error = shell
            .bind_session(
                session,
                MobileViewport {
                    width: 390.0,
                    height: 844.0,
                    scale_factor: 3.0,
                },
            )
            .expect_err("platform mismatch should fail");

        assert_eq!(
            error.error_code(),
            ZenoErrorCode::MobileSessionPlatformMismatch
        );
    }

    #[test]
    fn prepare_app_session_resolves_backend_before_binding() {
        let shell = MobileShell {
            platform: MobilePlatform::Android,
        };
        let binding = shell
            .prepare_app_session(
                &AppConfig {
                    renderer: RendererConfig::default(),
                    ..AppConfig::default()
                },
                MobileViewport {
                    width: 412.0,
                    height: 915.0,
                    scale_factor: 2.75,
                },
            )
            .expect("android session binding");

        assert_eq!(binding.platform, MobilePlatform::Android);
        assert_eq!(binding.session.platform, Platform::Android);
        assert_eq!(binding.backend, Backend::Skia);
        assert_eq!(binding.presenter, MobilePresenterKind::SkiaSurface);
        assert_eq!(binding.session.window.size.width, 412.0);
        assert_eq!(binding.session.window.size.height, 915.0);
        assert_eq!(binding.surface.surface.platform, Platform::Android);
    }

    #[test]
    fn attach_session_accepts_android_native_window() {
        let shell = MobileShell {
            platform: MobilePlatform::Android,
        };
        let attached = shell
            .prepare_attached_app_session(
                &AppConfig::default(),
                MobileViewport {
                    width: 412.0,
                    height: 915.0,
                    scale_factor: 2.75,
                },
                MobileAttachContext::AndroidSurface(AndroidAttachContext {
                    native_window: fake_handle(1),
                }),
            )
            .expect("android attached session");

        assert_eq!(attached.binding.platform, MobilePlatform::Android);
        assert_eq!(
            attached.attachment,
            MobilePresenterAttachment {
                host_kind: MobileHostKind::AndroidNativeWindow,
                presenter: MobilePresenterKind::SkiaSurface,
            }
        );
    }

    #[test]
    fn attach_session_rejects_attach_platform_mismatch() {
        let shell = MobileShell {
            platform: MobilePlatform::Android,
        };
        let binding = shell
            .prepare_app_session(
                &AppConfig::default(),
                MobileViewport {
                    width: 412.0,
                    height: 915.0,
                    scale_factor: 2.75,
                },
            )
            .expect("android binding");
        let error = shell
            .attach_session(
                binding,
                MobileAttachContext::IosView(IosViewAttachContext {
                    ui_view: fake_handle(2),
                }),
            )
            .expect_err("platform mismatch should fail");

        assert_eq!(error.error_code(), ZenoErrorCode::MobileAttachPlatformMismatch);
    }

    #[test]
    fn attach_session_rejects_impeller_without_required_host() {
        let shell = MobileShell {
            platform: MobilePlatform::Ios,
        };
        let binding = shell
            .bind_session(
                ResolvedSession::new(
                    Platform::Ios,
                    WindowConfig::default(),
                    ResolvedBackend {
                        backend_kind: Backend::Impeller,
                        attempts: vec![BackendAttempt {
                            backend: Backend::Impeller,
                            reason: None,
                        }],
                    },
                    false,
                ),
                MobileViewport {
                    width: 390.0,
                    height: 844.0,
                    scale_factor: 3.0,
                },
            )
            .expect("ios impeller binding");
        let error = shell
            .attach_session(
                binding,
                MobileAttachContext::IosView(IosViewAttachContext {
                    ui_view: fake_handle(3),
                }),
            )
            .expect_err("impeller should require a metal layer host");

        assert_eq!(
            error.error_code(),
            ZenoErrorCode::BackendMissingPlatformSurface
        );
    }

    #[test]
    fn attach_session_accepts_ios_metal_layer_for_impeller() {
        let shell = MobileShell {
            platform: MobilePlatform::Ios,
        };
        let binding = shell
            .bind_session(
                ResolvedSession::new(
                    Platform::Ios,
                    WindowConfig::default(),
                    ResolvedBackend {
                        backend_kind: Backend::Impeller,
                        attempts: vec![BackendAttempt {
                            backend: Backend::Impeller,
                            reason: None,
                        }],
                    },
                    false,
                ),
                MobileViewport {
                    width: 390.0,
                    height: 844.0,
                    scale_factor: 3.0,
                },
            )
            .expect("ios impeller binding");
        let attached = shell
            .attach_session(
                binding,
                MobileAttachContext::IosMetalLayer(IosMetalLayerAttachContext {
                    metal_layer: fake_handle(4),
                    ui_view: Some(fake_handle(5)),
                }),
            )
            .expect("impeller metal attachment");

        assert_eq!(attached.attachment.host_kind, MobileHostKind::IosMetalLayer);
        assert_eq!(attached.attachment.presenter, MobilePresenterKind::ImpellerSurface);
    }

    #[test]
    fn create_render_session_builds_android_session() {
        let shell = MobileShell {
            platform: MobilePlatform::Android,
        };
        let attached = shell
            .prepare_attached_app_session(
                &AppConfig::default(),
                MobileViewport {
                    width: 412.0,
                    height: 915.0,
                    scale_factor: 2.75,
                },
                MobileAttachContext::AndroidSurface(AndroidAttachContext {
                    native_window: fake_handle(6),
                }),
            )
            .expect("android attached session");
        let mut session = create_mobile_render_session(attached).expect("android render session");
        let report = session.submit_scene(&test_submit()).expect("submit scene");

        assert_eq!(session.kind(), Backend::Skia);
        assert_eq!(session.attachment().host_kind, MobileHostKind::AndroidNativeWindow);
        assert_eq!(report.backend, Backend::Skia);
        assert_eq!(report.command_count, 1);
    }

    #[test]
    fn prepare_render_session_builds_android_skia_session() {
        let shell = MobileShell {
            platform: MobilePlatform::Android,
        };
        let mut session = shell
            .prepare_render_session(
                &AppConfig::default(),
                MobileViewport {
                    width: 412.0,
                    height: 915.0,
                    scale_factor: 2.75,
                },
                MobileAttachContext::AndroidSurface(AndroidAttachContext {
                    native_window: fake_handle(7),
                }),
            )
            .expect("android render session");
        session.resize(800, 600).expect("resize mobile session");
        let report = session.submit_scene(&test_submit()).expect("submit scene");

        assert_eq!(session.kind(), Backend::Skia);
        assert_eq!(session.surface().size.width, 800.0);
        assert_eq!(session.surface().size.height, 600.0);
        assert_eq!(session.attachment().host_kind, MobileHostKind::AndroidNativeWindow);
        assert_eq!(report.backend, Backend::Skia);
    }

    #[test]
    fn create_render_session_builds_ios_impeller_session() {
        let shell = MobileShell {
            platform: MobilePlatform::Ios,
        };
        let attached = shell
            .attach_session(
                shell.bind_session(
                    ResolvedSession::new(
                        Platform::Ios,
                        WindowConfig::default(),
                        ResolvedBackend {
                            backend_kind: Backend::Impeller,
                            attempts: vec![BackendAttempt {
                                backend: Backend::Impeller,
                                reason: None,
                            }],
                        },
                        false,
                    ),
                    MobileViewport {
                        width: 390.0,
                        height: 844.0,
                        scale_factor: 3.0,
                    },
                )
                .expect("ios impeller binding"),
                MobileAttachContext::IosMetalLayer(IosMetalLayerAttachContext {
                    metal_layer: fake_handle(8),
                    ui_view: Some(fake_handle(9)),
                }),
            )
            .expect("ios attached session");
        let mut session = shell
            .create_render_session(attached)
            .expect("ios render session");
        session.resize(800, 600).expect("resize mobile session");
        let report = session.submit_scene(&test_submit()).expect("submit scene");

        assert_eq!(session.kind(), Backend::Impeller);
        assert_eq!(session.surface().size.width, 800.0);
        assert_eq!(session.surface().size.height, 600.0);
        assert_eq!(session.attachment().host_kind, MobileHostKind::IosMetalLayer);
        assert_eq!(report.backend, Backend::Impeller);
    }
}
