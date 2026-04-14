#[cfg(feature = "mobile_android")]
use std::num::NonZeroUsize;

#[cfg(feature = "mobile_android")]
use zeno_backend_skia::SkiaBackend;
use zeno_core::Platform;
#[cfg(feature = "mobile_android")]
use zeno_core::{Backend, ZenoError, ZenoErrorCode};
#[cfg(feature = "mobile_android")]
use zeno_scene::{
    DisplayList, FrameReport, GraphicsBackend, RenderCapabilities, RenderSurface, Renderer,
};

use crate::PlatformDescriptor;

#[must_use]
pub fn descriptor() -> PlatformDescriptor {
    PlatformDescriptor {
        platform: Platform::Android,
        impeller_preferred: true,
        notes: "android surface shell with native renderer handoff",
    }
}

#[cfg(feature = "mobile_android")]
pub(crate) enum AndroidMobilePresenter {
    SkiaNativeWindow(AndroidSkiaNativeWindowPresenter),
    ImpellerNativeWindow(AndroidImpellerNativeWindowPresenter),
}

#[cfg(feature = "mobile_android")]
impl AndroidMobilePresenter {
    pub(crate) fn create_skia_native_window(
        native_window: NonZeroUsize,
    ) -> Result<Self, ZenoError> {
        Ok(Self::SkiaNativeWindow(
            AndroidSkiaNativeWindowPresenter::new(native_window)?,
        ))
    }

    pub(crate) fn create_impeller_native_window(
        native_window: NonZeroUsize,
    ) -> Result<Self, ZenoError> {
        Ok(Self::ImpellerNativeWindow(
            AndroidImpellerNativeWindowPresenter::new(native_window)?,
        ))
    }

    pub(crate) fn kind(&self) -> Backend {
        match self {
            Self::SkiaNativeWindow(presenter) => presenter.kind(),
            Self::ImpellerNativeWindow(presenter) => presenter.kind(),
        }
    }

    pub(crate) fn capabilities(&self) -> RenderCapabilities {
        match self {
            Self::SkiaNativeWindow(presenter) => presenter.capabilities(),
            Self::ImpellerNativeWindow(presenter) => presenter.capabilities(),
        }
    }

    pub(crate) fn render_display_list(
        &self,
        surface: &RenderSurface,
        display_list: &DisplayList,
    ) -> Result<FrameReport, ZenoError> {
        match self {
            Self::SkiaNativeWindow(presenter) => {
                presenter.render_display_list(surface, display_list)
            }
            Self::ImpellerNativeWindow(presenter) => {
                presenter.render_display_list(surface, display_list)
            }
        }
    }
}

#[cfg(feature = "mobile_android")]
pub(crate) struct AndroidSkiaNativeWindowPresenter {
    native_window: NonZeroUsize,
    renderer: Box<dyn Renderer>,
}

#[cfg(feature = "mobile_android")]
impl AndroidSkiaNativeWindowPresenter {
    fn new(native_window: NonZeroUsize) -> Result<Self, ZenoError> {
        Ok(Self {
            native_window,
            renderer: SkiaBackend.create_renderer().map_err(|error| {
                ZenoError::invalid_configuration(
                    ZenoErrorCode::SessionCreateRenderSessionFailed,
                    "shell.platform.android",
                    "create_skia_presenter",
                    error.message().into_owned(),
                )
            })?,
        })
    }

    fn kind(&self) -> Backend {
        let _ = self.native_window;
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

#[cfg(feature = "mobile_android")]
pub(crate) struct AndroidImpellerNativeWindowPresenter {
    native_window: NonZeroUsize,
    renderer: Box<dyn Renderer>,
}

#[cfg(feature = "mobile_android")]
impl AndroidImpellerNativeWindowPresenter {
    fn new(_native_window: NonZeroUsize) -> Result<Self, ZenoError> {
        Err(ZenoError::BackendUnavailable {
            backend: Backend::Impeller,
            reason: zeno_core::BackendUnavailableReason::NotImplementedForPlatform,
        })
    }

    fn kind(&self) -> Backend {
        let _ = self.native_window;
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
