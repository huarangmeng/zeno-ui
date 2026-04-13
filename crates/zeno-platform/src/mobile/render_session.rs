use zeno_core::{Backend, Size, ZenoError, ZenoErrorCode};
use zeno_scene::{
    DisplayList, FrameReport, RenderCapabilities, RenderSession, RenderSurface, RetainedScene,
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

    fn submit_retained_scene(
        &mut self,
        scene: &mut RetainedScene,
        _dirty_bounds: Option<zeno_core::Rect>,
        patch_upserts: usize,
        patch_removes: usize,
    ) -> Result<FrameReport, ZenoError> {
        match self {
            Self::Android(session) => session.submit_retained_scene(scene, patch_upserts, patch_removes),
            Self::IosView(session) => session.submit_retained_scene(scene, patch_upserts, patch_removes),
            Self::IosMetalLayer(session) => session.submit_retained_scene(scene, patch_upserts, patch_removes),
        }
    }

    fn submit_display_list(
        &mut self,
        _display_list: &DisplayList,
        _dirty_bounds: Option<zeno_core::Rect>,
        _patch_upserts: usize,
        _patch_removes: usize,
    ) -> Result<FrameReport, ZenoError> {
        Err(mobile_session_error(
            ZenoErrorCode::WindowRendererUnavailable,
            "submit_display_list",
            "display list submit is not implemented for mobile sessions",
        ))
    }
}

pub(crate) struct AndroidNativeWindowSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    presenter: platform::android::AndroidMobilePresenter,
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

    fn submit_retained_scene(
        &mut self,
        scene: &mut RetainedScene,
        patch_upserts: usize,
        patch_removes: usize,
    ) -> Result<FrameReport, ZenoError> {
        submit_mobile_retained_scene(
            self.presenter.kind(),
            self.presenter.capabilities(),
            |surface, snapshot| self.presenter.render(surface, snapshot),
            &self.surface,
            scene,
            patch_upserts,
            patch_removes,
        )
    }
}

pub(crate) struct IosViewSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    presenter: platform::ios::IosMobilePresenter,
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

    fn submit_retained_scene(
        &mut self,
        scene: &mut RetainedScene,
        patch_upserts: usize,
        patch_removes: usize,
    ) -> Result<FrameReport, ZenoError> {
        submit_mobile_retained_scene(
            self.presenter.kind(),
            self.presenter.capabilities(),
            |surface, snapshot| self.presenter.render(surface, snapshot),
            &self.surface,
            scene,
            patch_upserts,
            patch_removes,
        )
    }
}

pub(crate) struct IosMetalLayerSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    presenter: platform::ios::IosMobilePresenter,
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

    fn submit_retained_scene(
        &mut self,
        scene: &mut RetainedScene,
        patch_upserts: usize,
        patch_removes: usize,
    ) -> Result<FrameReport, ZenoError> {
        submit_mobile_retained_scene(
            self.presenter.kind(),
            self.presenter.capabilities(),
            |surface, snapshot| self.presenter.render(surface, snapshot),
            &self.surface,
            scene,
            patch_upserts,
            patch_removes,
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

fn submit_mobile_retained_scene(
    backend: Backend,
    capabilities: RenderCapabilities,
    render_scene: impl FnOnce(
        &RenderSurface,
        &mut RetainedScene,
    ) -> Result<FrameReport, ZenoError>,
    surface: &RenderSurface,
    scene: &mut RetainedScene,
    patch_upserts: usize,
    patch_removes: usize,
) -> Result<FrameReport, ZenoError> {
    let mut report = render_scene(surface, scene)?;
    report.backend = backend;
    report.command_count = scene.packet_count();
    report.resource_count = scene.resource_key_count();
    report.block_count = scene.live_object_count();
    report.patch_upserts = patch_upserts;
    report.patch_removes = patch_removes;
    report.surface_id = surface.id.clone();
    let _ = capabilities;
    Ok(report)
}
