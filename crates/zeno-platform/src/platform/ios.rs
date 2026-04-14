#[cfg(feature = "mobile_ios")]
use std::num::NonZeroUsize;

#[cfg(feature = "mobile_ios")]
use zeno_backend_skia::SkiaBackend;
use zeno_core::Platform;
#[cfg(feature = "mobile_ios")]
use zeno_core::{Backend, ZenoError, ZenoErrorCode};
#[cfg(feature = "mobile_ios")]
use zeno_scene::{
    DisplayList, FrameReport, GraphicsBackend, RenderCapabilities, RenderSurface, Renderer,
};

use crate::PlatformDescriptor;

#[must_use]
pub fn descriptor() -> PlatformDescriptor {
    PlatformDescriptor {
        platform: Platform::Ios,
        impeller_preferred: true,
        notes: "uiview shell with metal layer",
    }
}

#[cfg(feature = "mobile_ios")]
pub(crate) enum IosMobilePresenter {
    SkiaView(IosSkiaViewPresenter),
    SkiaMetalLayer(IosSkiaMetalLayerPresenter),
    ImpellerMetalLayer(IosImpellerMetalLayerPresenter),
}

#[cfg(feature = "mobile_ios")]
impl IosMobilePresenter {
    pub(crate) fn create_skia_view(ui_view: NonZeroUsize) -> Result<Self, ZenoError> {
        Ok(Self::SkiaView(IosSkiaViewPresenter::new(ui_view)?))
    }

    pub(crate) fn create_skia_metal_layer(
        metal_layer: NonZeroUsize,
        ui_view: Option<NonZeroUsize>,
    ) -> Result<Self, ZenoError> {
        Ok(Self::SkiaMetalLayer(IosSkiaMetalLayerPresenter::new(
            metal_layer,
            ui_view,
        )?))
    }

    pub(crate) fn create_impeller_metal_layer(
        metal_layer: NonZeroUsize,
        ui_view: Option<NonZeroUsize>,
    ) -> Result<Self, ZenoError> {
        Ok(Self::ImpellerMetalLayer(
            IosImpellerMetalLayerPresenter::new(metal_layer, ui_view)?,
        ))
    }

    pub(crate) fn kind(&self) -> Backend {
        match self {
            Self::SkiaView(presenter) => presenter.kind(),
            Self::SkiaMetalLayer(presenter) => presenter.kind(),
            Self::ImpellerMetalLayer(presenter) => presenter.kind(),
        }
    }

    pub(crate) fn capabilities(&self) -> RenderCapabilities {
        match self {
            Self::SkiaView(presenter) => presenter.capabilities(),
            Self::SkiaMetalLayer(presenter) => presenter.capabilities(),
            Self::ImpellerMetalLayer(presenter) => presenter.capabilities(),
        }
    }

    pub(crate) fn render_display_list(
        &self,
        surface: &RenderSurface,
        display_list: &DisplayList,
    ) -> Result<FrameReport, ZenoError> {
        match self {
            Self::SkiaView(presenter) => presenter.render_display_list(surface, display_list),
            Self::SkiaMetalLayer(presenter) => presenter.render_display_list(surface, display_list),
            Self::ImpellerMetalLayer(presenter) => {
                presenter.render_display_list(surface, display_list)
            }
        }
    }
}

#[cfg(feature = "mobile_ios")]
pub(crate) struct IosSkiaViewPresenter {
    ui_view: NonZeroUsize,
    renderer: Box<dyn Renderer>,
}

#[cfg(feature = "mobile_ios")]
impl IosSkiaViewPresenter {
    fn new(ui_view: NonZeroUsize) -> Result<Self, ZenoError> {
        Ok(Self {
            ui_view,
            renderer: SkiaBackend.create_renderer().map_err(|error| {
                ZenoError::invalid_configuration(
                    ZenoErrorCode::SessionCreateRenderSessionFailed,
                    "shell.platform.ios",
                    "create_skia_view_presenter",
                    error.message().into_owned(),
                )
            })?,
        })
    }

    fn kind(&self) -> Backend {
        let _ = self.ui_view;
        Backend::Skia
    }

    fn capabilities(&self) -> RenderCapabilities {
        self.renderer.capabilities()
    }

    fn render_display_list(
        &self,
        surface: &RenderSurface,
        display_list: &DisplayList,
    ) -> Result<FrameReport, ZenoError> {
        self.renderer.render_display_list(surface, display_list)
    }
}

#[cfg(feature = "mobile_ios")]
pub(crate) struct IosSkiaMetalLayerPresenter {
    metal_layer: NonZeroUsize,
    ui_view: Option<NonZeroUsize>,
    renderer: Box<dyn Renderer>,
}

#[cfg(feature = "mobile_ios")]
impl IosSkiaMetalLayerPresenter {
    fn new(metal_layer: NonZeroUsize, ui_view: Option<NonZeroUsize>) -> Result<Self, ZenoError> {
        Ok(Self {
            metal_layer,
            ui_view,
            renderer: SkiaBackend.create_renderer().map_err(|error| {
                ZenoError::invalid_configuration(
                    ZenoErrorCode::SessionCreateRenderSessionFailed,
                    "shell.platform.ios",
                    "create_skia_metal_presenter",
                    error.message().into_owned(),
                )
            })?,
        })
    }

    fn kind(&self) -> Backend {
        let _ = self.metal_layer;
        let _ = self.ui_view;
        Backend::Skia
    }

    fn capabilities(&self) -> RenderCapabilities {
        self.renderer.capabilities()
    }

    fn render_display_list(
        &self,
        surface: &RenderSurface,
        display_list: &DisplayList,
    ) -> Result<FrameReport, ZenoError> {
        self.renderer.render_display_list(surface, display_list)
    }
}

#[cfg(feature = "mobile_ios")]
pub(crate) struct IosImpellerMetalLayerPresenter {
    metal_layer: NonZeroUsize,
    ui_view: Option<NonZeroUsize>,
    renderer: Box<dyn Renderer>,
}

#[cfg(feature = "mobile_ios")]
impl IosImpellerMetalLayerPresenter {
    fn new(_metal_layer: NonZeroUsize, _ui_view: Option<NonZeroUsize>) -> Result<Self, ZenoError> {
        Err(ZenoError::BackendUnavailable {
            backend: Backend::Impeller,
            reason: zeno_core::BackendUnavailableReason::NotImplementedForPlatform,
        })
    }

    fn kind(&self) -> Backend {
        let _ = self.metal_layer;
        let _ = self.ui_view;
        Backend::Impeller
    }

    fn capabilities(&self) -> RenderCapabilities {
        self.renderer.capabilities()
    }

    fn render_display_list(
        &self,
        surface: &RenderSurface,
        display_list: &DisplayList,
    ) -> Result<FrameReport, ZenoError> {
        self.renderer.render_display_list(surface, display_list)
    }
}
