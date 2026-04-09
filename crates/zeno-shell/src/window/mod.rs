use zeno_core::{
    AppConfig, Backend, Platform, WindowConfig, ZenoError, ZenoErrorCode,
};
use zeno_graphics::{DrawCommand, Scene, SceneSubmit};
use zeno_runtime::{BackendAttempt, ResolvedBackend, ResolvedSession};

#[cfg(feature = "desktop_winit")]
mod app;
#[cfg(feature = "desktop_winit")]
use app::DesktopWindowApp;

use crate::shell::{DesktopShell, Shell};

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

impl DesktopShell {
    #[cfg(feature = "desktop_winit")]
    pub fn run_window(&self, config: &WindowConfig) -> Result<(), ZenoError> {
        self.run_pending_scene_window(
            ResolvedSession::new(
                Platform::current(),
                config.clone(),
                ResolvedBackend {
                    backend_kind: Backend::Skia,
                    attempts: Vec::new(),
                },
                false,
            ),
            SceneSubmit::Full(Scene {
                size: config.size,
                clear_color: Some(zeno_core::Color::WHITE),
                commands: vec![DrawCommand::Clear(zeno_core::Color::WHITE)],
                layers: vec![zeno_graphics::SceneLayer::root(config.size)],
                blocks: Vec::new(),
            }),
        )
    }

    #[cfg(feature = "desktop_winit")]
    pub fn run_scene_window(&self, config: &WindowConfig, scene: Scene) -> Result<(), ZenoError> {
        self.run_pending_scene_window(
            ResolvedSession::new(
                Platform::current(),
                config.clone(),
                ResolvedBackend {
                    backend_kind: Backend::Skia,
                    attempts: Vec::new(),
                },
                false,
            ),
            SceneSubmit::Full(scene),
        )
    }

    #[cfg(feature = "desktop_winit")]
    pub fn run_app_scene_window(
        &self,
        app_config: &AppConfig,
        scene: Scene,
    ) -> Result<ResolvedWindowRun, ZenoError> {
        let session = self.prepare_app_window_session(app_config)?;
        let outcome = ResolvedWindowRun {
            backend: session.backend.backend_kind,
            attempts: session.backend.attempts.clone(),
        };
        self.run_pending_scene_window(session, SceneSubmit::Full(scene))?;
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
    pub fn run_pending_scene_window(
        &self,
        pending: ResolvedSession,
        submit: SceneSubmit,
    ) -> Result<(), ZenoError> {
        self.run_window_session(pending, submit)
    }

    #[cfg(feature = "desktop_winit")]
    fn run_window_session(
        &self,
        resolved_session: ResolvedSession,
        submit: SceneSubmit,
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
        let mut app = DesktopWindowApp::new(resolved_session, submit);
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
