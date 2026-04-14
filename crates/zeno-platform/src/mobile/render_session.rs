use zeno_core::{Backend, Size, ZenoError, ZenoErrorCode};
use zeno_scene::{
    CompositorFrame, CompositorSubmission, DisplayList, FrameReport, RenderCapabilities,
    RenderSession, RenderSurface, TileCache,
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

    fn submit_compositor_frame(
        &mut self,
        frame: &CompositorFrame<DisplayList>,
    ) -> Result<FrameReport, ZenoError> {
        let display_list = &frame.payload;
        match self {
            Self::Android(session) => {
                session.submit_display_list(
                    display_list,
                    frame.damage.rect_count(),
                    frame.damage.is_full(),
                    &frame.damage,
                )
            }
            Self::IosView(session) => {
                session.submit_display_list(
                    display_list,
                    frame.damage.rect_count(),
                    frame.damage.is_full(),
                    &frame.damage,
                )
            }
            Self::IosMetalLayer(session) => {
                session.submit_display_list(
                    display_list,
                    frame.damage.rect_count(),
                    frame.damage.is_full(),
                    &frame.damage,
                )
            }
        }
    }
}

pub(crate) struct AndroidNativeWindowSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    presenter: platform::android::AndroidMobilePresenter,
    tile_cache: TileCache,
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
            tile_cache: TileCache::new(),
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

    fn submit_display_list(
        &mut self,
        display_list: &DisplayList,
        damage_rect_count: usize,
        damage_full: bool,
        damage: &zeno_scene::DamageRegion,
    ) -> Result<FrameReport, ZenoError> {
        let submission = display_list.build_compositor_submission(&mut self.tile_cache, damage);
        submit_mobile_display_list(
            self.presenter.kind(),
            self.presenter.capabilities(),
            |surface, list| self.presenter.render_display_list(surface, list),
            &self.surface,
            display_list,
            damage_rect_count,
            damage_full,
            &submission,
        )
    }
}

pub(crate) struct IosViewSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    presenter: platform::ios::IosMobilePresenter,
    tile_cache: TileCache,
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
            tile_cache: TileCache::new(),
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

    fn submit_display_list(
        &mut self,
        display_list: &DisplayList,
        damage_rect_count: usize,
        damage_full: bool,
        damage: &zeno_scene::DamageRegion,
    ) -> Result<FrameReport, ZenoError> {
        let submission = display_list.build_compositor_submission(&mut self.tile_cache, damage);
        submit_mobile_display_list(
            self.presenter.kind(),
            self.presenter.capabilities(),
            |surface, list| self.presenter.render_display_list(surface, list),
            &self.surface,
            display_list,
            damage_rect_count,
            damage_full,
            &submission,
        )
    }
}

pub(crate) struct IosMetalLayerSession {
    backend: Backend,
    attachment: MobilePresenterAttachment,
    surface: RenderSurface,
    presenter: platform::ios::IosMobilePresenter,
    tile_cache: TileCache,
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
            tile_cache: TileCache::new(),
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

    fn submit_display_list(
        &mut self,
        display_list: &DisplayList,
        damage_rect_count: usize,
        damage_full: bool,
        damage: &zeno_scene::DamageRegion,
    ) -> Result<FrameReport, ZenoError> {
        let submission = display_list.build_compositor_submission(&mut self.tile_cache, damage);
        submit_mobile_display_list(
            self.presenter.kind(),
            self.presenter.capabilities(),
            |surface, list| self.presenter.render_display_list(surface, list),
            &self.surface,
            display_list,
            damage_rect_count,
            damage_full,
            &submission,
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

fn submit_mobile_display_list(
    backend: Backend,
    capabilities: RenderCapabilities,
    render_display_list: impl FnOnce(&RenderSurface, &DisplayList) -> Result<FrameReport, ZenoError>,
    surface: &RenderSurface,
    display_list: &DisplayList,
    damage_rect_count: usize,
    damage_full: bool,
    submission: &CompositorSubmission,
) -> Result<FrameReport, ZenoError> {
    let mut report = render_display_list(surface, display_list)?;
    report.backend = backend;
    report.command_count = display_list.items.len();
    report.display_item_count = display_list.items.len();
    report.stacking_context_count = display_list.stacking_contexts.len();
    report.damage_rect_count = damage_rect_count;
    report.damage_full = damage_full;
    report.dirty_tile_count = submission.tile_plan.stats.reraster_tile_count;
    report.cached_tile_count = submission.tile_plan.stats.cached_tile_count;
    report.reraster_tile_count = submission.tile_plan.stats.reraster_tile_count;
    report.raster_batch_tile_count = submission.raster_batch.tile_count();
    report.composite_tile_count = submission.composite_pass.tile_count();
    report.compositor_layer_count = submission.layer_tree.layer_count();
    report.offscreen_layer_count = submission.layer_tree.offscreen_layer_count();
    report.tile_content_handle_count = submission.composite_pass.tile_count();
    report.compositor_task_count = 0;
    report.compositor_queue_depth = 0;
    report.compositor_dropped_frame_count = 0;
    report.compositor_processed_frame_count = 0;
    report.released_tile_resource_count = 0;
    report.evicted_tile_resource_count = 0;
    report.budget_evicted_tile_resource_count = 0;
    report.age_evicted_tile_resource_count = 0;
    report.descriptor_limit_evicted_tile_resource_count = 0;
    report.reused_tile_resource_count = 0;
    report.reusable_tile_resource_count = 0;
    report.reusable_tile_resource_bytes = 0;
    report.tile_resource_reuse_budget_bytes = 0;
    report.compositor_worker_threaded = false;
    report.compositor_worker_alive = false;
    report.composite_executed_layer_count = 0;
    report.composite_executed_tile_count = 0;
    report.composite_offscreen_step_count = 0;
    report.surface_id = surface.id.clone();
    let _ = capabilities;
    Ok(report)
}
