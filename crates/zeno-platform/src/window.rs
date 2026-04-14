use std::time::Duration;

use zeno_core::{
    AppConfig, Backend, Platform, Point, Size, WindowConfig, ZenoError, ZenoErrorCode,
};
use zeno_scene::{DisplayList, FrameReport, RenderSession};

use crate::session::{BackendAttempt, ResolvedBackend, ResolvedSession};
#[cfg(feature = "desktop_winit")]
mod runtime;
#[cfg(feature = "desktop_winit")]
use runtime::DesktopWindowApp;

use crate::shell::{DesktopShell, NativeSurfaceHostRequirement, Shell, create_native_surface};

#[derive(Debug, Clone, PartialEq)]
pub struct DesktopWindowHandle {
    pub id: String,
    pub title: String,
    pub size: (u32, u32),
    pub surface: crate::NativeSurface,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedWindowRun {
    pub backend: Backend,
    pub attempts: Vec<BackendAttempt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointerState {
    pub position: Option<Point>,
    pub pressed: bool,
    pub just_pressed: bool,
    pub just_released: bool,
}

impl Default for PointerState {
    fn default() -> Self {
        Self {
            position: None,
            pressed: false,
            just_pressed: false,
            just_released: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimatedFrameContext {
    pub frame_index: u64,
    pub elapsed: Duration,
    pub delta: Duration,
    pub size: Size,
    pub backend: Backend,
    pub last_report: Option<FrameReport>,
    pub pointer: PointerState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameRequest {
    Wait,
    NextFrame,
    After(Duration),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimatedFrameOutput {
    pub report: Option<FrameReport>,
    pub frame_request: FrameRequest,
}

impl AnimatedFrameOutput {
    #[must_use]
    pub fn submitted(report: FrameReport, frame_request: FrameRequest) -> Self {
        Self {
            report: Some(report),
            frame_request,
        }
    }
}

#[cfg(feature = "desktop_winit")]
pub type BoxedAnimatedSceneCallback = Box<
    dyn FnMut(
        AnimatedFrameContext,
        &mut dyn RenderSession,
    ) -> Result<AnimatedFrameOutput, ZenoError>,
>;

impl DesktopShell {
    #[cfg(feature = "desktop_winit")]
    pub fn run_window(&self, config: &WindowConfig) -> Result<(), ZenoError> {
        self.run_pending_display_list_window(
            ResolvedSession::new(
                Platform::current(),
                config.clone(),
                ResolvedBackend {
                    backend_kind: Backend::Skia,
                    attempts: Vec::new(),
                },
                false,
            ),
            DisplayList::empty(config.size),
        )
    }

    #[cfg(feature = "desktop_winit")]
    pub fn run_display_list_window(
        &self,
        config: &WindowConfig,
        display_list: DisplayList,
    ) -> Result<(), ZenoError> {
        self.run_pending_display_list_window(
            ResolvedSession::new(
                Platform::current(),
                config.clone(),
                ResolvedBackend {
                    backend_kind: Backend::Skia,
                    attempts: Vec::new(),
                },
                false,
            ),
            display_list,
        )
    }

    #[cfg(feature = "desktop_winit")]
    pub fn run_app_display_list_window(
        &self,
        app_config: &AppConfig,
        display_list: DisplayList,
    ) -> Result<ResolvedWindowRun, ZenoError> {
        let session = self.prepare_app_window_session(app_config)?;
        let outcome = ResolvedWindowRun {
            backend: session.backend.backend_kind,
            attempts: session.backend.attempts.clone(),
        };
        self.run_pending_display_list_window(session, display_list)?;
        Ok(outcome)
    }

    #[cfg(feature = "desktop_winit")]
    pub fn prepare_app_window_session(
        &self,
        app_config: &AppConfig,
    ) -> Result<ResolvedSession, ZenoError> {
        let native_surface = Shell::create_surface(self, &app_config.window);
        ResolvedSession::resolve(native_surface.descriptor.platform, app_config)
    }

    #[cfg(feature = "desktop_winit")]
    pub fn run_pending_display_list_window(
        &self,
        pending: ResolvedSession,
        display_list: DisplayList,
    ) -> Result<(), ZenoError> {
        self.run_window_session(pending, display_list)
    }

    #[cfg(feature = "desktop_winit")]
    #[doc(hidden)]
    pub fn run_animated_scene_window<F>(
        &self,
        pending: ResolvedSession,
        animator: F,
    ) -> Result<(), ZenoError>
    where
        F: FnMut(
                AnimatedFrameContext,
                &mut dyn RenderSession,
            ) -> Result<AnimatedFrameOutput, ZenoError>
            + 'static,
    {
        self.run_animated_window_session(pending, Box::new(animator))
    }

    #[cfg(feature = "desktop_winit")]
    fn run_window_session(
        &self,
        resolved_session: ResolvedSession,
        display_list: DisplayList,
    ) -> Result<(), ZenoError> {
        use winit::event_loop::{ControlFlow, EventLoop};

        let event_loop = EventLoop::new().map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::WindowCreateEventLoopFailed,
                "shell.window",
                "create_event_loop",
                error.to_string(),
            )
        })?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let native_surface = create_native_surface(
            &resolved_session.window,
            None,
            Some(resolved_session.backend.backend_kind),
            NativeSurfaceHostRequirement::DesktopWindow,
        );
        let mut app = DesktopWindowApp::new(resolved_session, native_surface, display_list);
        event_loop.run_app(&mut app).map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::WindowRunAppFailed,
                "shell.window",
                "run_app",
                error.to_string(),
            )
        })?;
        if let Some(error) = app.into_creation_error() {
            return Err(error);
        }
        Ok(())
    }

    #[cfg(feature = "desktop_winit")]
    fn run_animated_window_session(
        &self,
        resolved_session: ResolvedSession,
        animator: BoxedAnimatedSceneCallback,
    ) -> Result<(), ZenoError> {
        use winit::event_loop::{ControlFlow, EventLoop};

        let event_loop = EventLoop::new().map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::WindowCreateEventLoopFailed,
                "shell.window",
                "create_event_loop",
                error.to_string(),
            )
        })?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let native_surface = create_native_surface(
            &resolved_session.window,
            None,
            Some(resolved_session.backend.backend_kind),
            NativeSurfaceHostRequirement::DesktopWindow,
        );
        let mut app = DesktopWindowApp::new_animated(resolved_session, native_surface, animator);
        event_loop.run_app(&mut app).map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::WindowRunAppFailed,
                "shell.window",
                "run_app",
                error.to_string(),
            )
        })?;
        if let Some(error) = app.into_creation_error() {
            return Err(error);
        }
        Ok(())
    }

    #[cfg(not(feature = "desktop_winit"))]
    pub fn run_window(&self, _config: &WindowConfig) -> Result<(), ZenoError> {
        Err(ZenoError::invalid_configuration(
            ZenoErrorCode::WindowFeatureDisabled,
            "shell.window",
            "run_window",
            "desktop_winit feature is disabled",
        ))
    }
}
