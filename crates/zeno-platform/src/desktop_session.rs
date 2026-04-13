use crate::NativeSurface;
#[cfg(feature = "desktop_winit")]
use winit::event_loop::ActiveEventLoop;
#[cfg(feature = "desktop_winit")]
use winit::window::Window;
use zeno_core::{Backend, ZenoError, ZenoErrorCode};
use zeno_scene::{
    DisplayList, FrameReport, RenderCapabilities, RenderSession, RenderSurface, RetainedScene,
};
use crate::session::ResolvedSession;

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
mod impeller_metal;
mod session_plan;
mod scene;
#[cfg(feature = "desktop_winit")]
mod skia_gl;

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
use impeller_metal::ImpellerMetalSession;
use session_plan::DesktopSessionPlan;
#[cfg(feature = "desktop_winit")]
use skia_gl::SkiaGlSession;

#[cfg(feature = "desktop_winit")]
pub trait DesktopRenderSessionHandle: RenderSession {
    fn window(&self) -> &Window;

    fn cache_summary(&self) -> String;
}

#[cfg(feature = "desktop_winit")]
pub type BoxedDesktopRenderSession = Box<dyn DesktopRenderSessionHandle>;

pub(crate) fn desktop_session_error(
    code: ZenoErrorCode,
    operation: &'static str,
    message: impl Into<String>,
) -> ZenoError {
    ZenoError::invalid_configuration(code, "shell.desktop_session", operation, message)
}

#[cfg(feature = "desktop_winit")]
pub fn create_desktop_render_session(
    resolved: &ResolvedSession,
    native_surface: &NativeSurface,
    event_loop: &ActiveEventLoop,
) -> Result<BoxedDesktopRenderSession, ZenoError> {
    DesktopSessionPlan::from_resolved(resolved, native_surface)?
        .build(event_loop, native_surface, &resolved.window)
        .map(|session| Box::new(session) as BoxedDesktopRenderSession)
        .map_err(|error| {
            desktop_session_error(
                ZenoErrorCode::SessionCreateRenderSessionFailed,
                "create_render_session",
                error,
            )
        })
}

#[cfg(feature = "desktop_winit")]
enum DesktopRenderSession {
    Skia(SkiaGlSession),
    #[cfg(target_os = "macos")]
    Impeller(ImpellerMetalSession),
}

#[cfg(feature = "desktop_winit")]
impl DesktopRenderSessionHandle for DesktopRenderSession {
    fn window(&self) -> &Window {
        match self {
            Self::Skia(session) => session.window(),
            #[cfg(target_os = "macos")]
            Self::Impeller(session) => session.window(),
        }
    }

    fn cache_summary(&self) -> String {
        match self {
            Self::Skia(session) => session.cache_summary(),
            #[cfg(target_os = "macos")]
            Self::Impeller(session) => session.cache_summary(),
        }
    }
}

#[cfg(feature = "desktop_winit")]
impl RenderSession for DesktopRenderSession {
    fn kind(&self) -> Backend {
        match self {
            Self::Skia(_) => Backend::Skia,
            #[cfg(target_os = "macos")]
            Self::Impeller(_) => Backend::Impeller,
        }
    }

    fn capabilities(&self) -> RenderCapabilities {
        match self {
            Self::Skia(_) => RenderCapabilities {
                gpu_compositing: true,
                text_shaping: true,
                filters: true,
                offscreen_rendering: true,
                display_list_submit: true,
            },
            #[cfg(target_os = "macos")]
            Self::Impeller(_) => RenderCapabilities {
                gpu_compositing: true,
                text_shaping: true,
                filters: true,
                offscreen_rendering: true,
                display_list_submit: true,
            },
        }
    }

    fn surface(&self) -> &RenderSurface {
        match self {
            Self::Skia(session) => session.surface(),
            #[cfg(target_os = "macos")]
            Self::Impeller(session) => session.surface(),
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError> {
        match self {
            Self::Skia(session) => session.resize(width, height),
            #[cfg(target_os = "macos")]
            Self::Impeller(session) => session.resize(width, height),
        }
    }

    fn submit_retained_scene(
        &mut self,
        scene: &mut RetainedScene,
        dirty_bounds: Option<zeno_core::Rect>,
        patch_upserts: usize,
        patch_removes: usize,
    ) -> Result<FrameReport, ZenoError> {
        match self {
            Self::Skia(session) => {
                session.submit_retained_scene(scene, dirty_bounds, patch_upserts, patch_removes)
            }
            #[cfg(target_os = "macos")]
            Self::Impeller(session) => {
                session.submit_retained_scene(scene, dirty_bounds, patch_upserts, patch_removes)
            }
        }
    }

    fn submit_display_list(
        &mut self,
        display_list: &DisplayList,
        dirty_bounds: Option<zeno_core::Rect>,
        patch_upserts: usize,
        patch_removes: usize,
    ) -> Result<FrameReport, ZenoError> {
        match self {
            Self::Skia(session) => {
                session.submit_display_list(display_list, dirty_bounds, patch_upserts, patch_removes)
            }
            #[cfg(target_os = "macos")]
            Self::Impeller(session) => session.submit_display_list(
                display_list,
                dirty_bounds,
                patch_upserts,
                patch_removes,
            ),
        }
    }
}
