use zeno_core::{Backend, Size, ZenoError, ZenoErrorCode};
use zeno_scene::{
    FrameReport, RenderCapabilities, RenderSession, RenderSurface, Scene, SceneSubmit,
};

use crate::platform;

use super::protocol::{
    MobileAttachContext, MobileAttachedSession, MobilePresenterAttachment,
    MobilePresenterInterface, MobileRenderSessionHandle,
};
use super::session_plan::{MobileSessionPlan, mobile_session_error};

pub type BoxedMobileRenderSession = Box<dyn MobileRenderSessionHandle>;

pub fn create_mobile_render_session(
    attached: MobileAttachedSession,
) -> Result<BoxedMobileRenderSession, ZenoError> {
    MobileSessionPlan::from_binding(&attached.binding)
        .build(attached)
        .map(|session| Box::new(session) as BoxedMobileRenderSession)
}

pub(crate) enum MobileRenderSession {
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

pub(crate) struct AndroidNativeWindowSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    presenter: platform::android::AndroidMobilePresenter,
    last_scene: Option<Scene>,
}

impl AndroidNativeWindowSession {
    pub(crate) fn new(attached: MobileAttachedSession) -> Result<Self, ZenoError> {
        let presenter = match (attached.attachment.interface, attached.context) {
            (
                MobilePresenterInterface::AndroidSkiaNativeWindow,
                MobileAttachContext::AndroidSurface(context),
            ) => platform::android::AndroidMobilePresenter::create_skia_native_window(
                context.native_window,
            )?,
            (
                MobilePresenterInterface::AndroidImpellerNativeWindow,
                MobileAttachContext::AndroidSurface(context),
            ) => platform::android::AndroidMobilePresenter::create_impeller_native_window(
                context.native_window,
            )?,
            _ => {
                return Err(mobile_session_error(
                    ZenoErrorCode::SessionCreateRenderSessionFailed,
                    "create_android_session",
                    "android session requires android native window presenter interface",
                ));
            }
        };
        Ok(Self {
            backend: attached.binding.backend,
            attachment: attached.attachment,
            surface: attached.binding.surface.surface,
            presenter,
            last_scene: None,
        })
    }

    fn kind(&self) -> Backend {
        self.backend
    }

    fn capabilities(&self) -> RenderCapabilities {
        self.presenter.capabilities()
    }

    fn surface(&self) -> &RenderSurface {
        &self.surface
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError> {
        resize_mobile_surface(&mut self.surface, width, height)
    }

    fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError> {
        let backend = self.presenter.kind();
        let capabilities = self.presenter.capabilities();
        submit_mobile_scene(
            backend,
            capabilities,
            |surface, scene| self.presenter.render(surface, scene),
            &self.surface,
            &mut self.last_scene,
            submit,
        )
    }
}

pub(crate) struct IosViewSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    presenter: platform::ios::IosMobilePresenter,
    last_scene: Option<Scene>,
}

impl IosViewSession {
    pub(crate) fn new(attached: MobileAttachedSession) -> Result<Self, ZenoError> {
        let presenter = match (attached.attachment.interface, attached.context) {
            (MobilePresenterInterface::IosSkiaView, MobileAttachContext::IosView(context)) => {
                platform::ios::IosMobilePresenter::create_skia_view(context.ui_view)?
            }
            _ => {
                return Err(mobile_session_error(
                    ZenoErrorCode::SessionCreateRenderSessionFailed,
                    "create_ios_view_session",
                    "ios view session requires ios skia view presenter interface",
                ));
            }
        };
        Ok(Self {
            backend: attached.binding.backend,
            attachment: attached.attachment,
            surface: attached.binding.surface.surface,
            presenter,
            last_scene: None,
        })
    }

    fn kind(&self) -> Backend {
        self.backend
    }

    fn capabilities(&self) -> RenderCapabilities {
        self.presenter.capabilities()
    }

    fn surface(&self) -> &RenderSurface {
        &self.surface
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError> {
        resize_mobile_surface(&mut self.surface, width, height)
    }

    fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError> {
        let backend = self.presenter.kind();
        let capabilities = self.presenter.capabilities();
        submit_mobile_scene(
            backend,
            capabilities,
            |surface, scene| self.presenter.render(surface, scene),
            &self.surface,
            &mut self.last_scene,
            submit,
        )
    }
}

pub(crate) struct IosMetalLayerSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    presenter: platform::ios::IosMobilePresenter,
    last_scene: Option<Scene>,
}

impl IosMetalLayerSession {
    pub(crate) fn new(attached: MobileAttachedSession) -> Result<Self, ZenoError> {
        let presenter = match (attached.attachment.interface, attached.context) {
            (
                MobilePresenterInterface::IosSkiaMetalLayer,
                MobileAttachContext::IosMetalLayer(context),
            ) => platform::ios::IosMobilePresenter::create_skia_metal_layer(
                context.metal_layer,
                context.ui_view,
            )?,
            (
                MobilePresenterInterface::IosImpellerMetalLayer,
                MobileAttachContext::IosMetalLayer(context),
            ) => platform::ios::IosMobilePresenter::create_impeller_metal_layer(
                context.metal_layer,
                context.ui_view,
            )?,
            _ => {
                return Err(mobile_session_error(
                    ZenoErrorCode::SessionCreateRenderSessionFailed,
                    "create_ios_metal_session",
                    "ios metal session requires metal-layer presenter interface",
                ));
            }
        };
        Ok(Self {
            backend: attached.binding.backend,
            attachment: attached.attachment,
            surface: attached.binding.surface.surface,
            presenter,
            last_scene: None,
        })
    }

    fn kind(&self) -> Backend {
        self.backend
    }

    fn capabilities(&self) -> RenderCapabilities {
        self.presenter.capabilities()
    }

    fn surface(&self) -> &RenderSurface {
        &self.surface
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError> {
        resize_mobile_surface(&mut self.surface, width, height)
    }

    fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError> {
        let backend = self.presenter.kind();
        let capabilities = self.presenter.capabilities();
        submit_mobile_scene(
            backend,
            capabilities,
            |surface, scene| self.presenter.render(surface, scene),
            &self.surface,
            &mut self.last_scene,
            submit,
        )
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
    capabilities: RenderCapabilities,
    render_scene: impl FnOnce(&RenderSurface, &Scene) -> Result<FrameReport, ZenoError>,
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
    let mut report = render_scene(surface, &scene)?;
    report.backend = backend;
    report.command_count = scene.commands.len();
    report.resource_count = scene.resource_keys().len();
    report.block_count = scene.blocks.len();
    report.patch_upserts = patch_upserts;
    report.patch_removes = patch_removes;
    report.surface_id = surface.id.clone();
    let _ = capabilities;
    *last_scene = Some(scene);
    Ok(report)
}

fn patch_stats(submit: &SceneSubmit) -> (usize, usize) {
    match submit {
        SceneSubmit::Full(scene) => (scene.blocks.len(), 0),
        SceneSubmit::Patch { patch, .. } => (
            patch.upserts.len()
                + patch.reorders.len()
                + patch.layer_upserts.len()
                + patch.layer_reorders.len(),
            patch.removes.len() + patch.layer_removes.len(),
        ),
    }
}
