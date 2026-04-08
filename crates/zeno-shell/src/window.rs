use zeno_core::{
    AppConfig, Backend, Platform, WindowConfig, ZenoError, ZenoErrorCode, zeno_window_error,
    zeno_frame_log, zeno_session_log,
};
use zeno_graphics::{DrawCommand, FrameReport, Scene, SceneSubmit};
use zeno_runtime::{BackendAttempt, FrameScheduler, ResolvedBackend, ResolvedSession};
#[cfg(feature = "desktop_winit")]
use crate::desktop_session::{create_desktop_render_session, BoxedDesktopRenderSession};

use crate::shell::{DesktopShell, Shell};

#[cfg(feature = "desktop_winit")]
use winit::application::ApplicationHandler;
#[cfg(feature = "desktop_winit")]
use winit::event::WindowEvent;
#[cfg(feature = "desktop_winit")]
use winit::event_loop::{ActiveEventLoop, ControlFlow};
#[cfg(feature = "desktop_winit")]
use winit::window::WindowId;

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
                commands: vec![DrawCommand::Clear(zeno_core::Color::WHITE)],
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
        use winit::event_loop::EventLoop;

        let event_loop = EventLoop::new()
            .map_err(|error| {
                ZenoError::invalid_configuration(
                    ZenoErrorCode::WindowCreateEventLoopFailed,
                    "shell.window",
                    "create_event_loop",
                    error.to_string(),
                )
            })?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut app = DesktopWindowApp {
            resolved_session,
            scene_submit: submit,
            session: None,
            scheduler: FrameScheduler::new(),
            frame_index: 0,
            last_report: None,
            window_id: None,
            creation_error: None,
        };
        event_loop
            .run_app(&mut app)
            .map_err(|error| {
                ZenoError::invalid_configuration(
                    ZenoErrorCode::WindowRunAppFailed,
                    "shell.window",
                    "run_app",
                    error.to_string(),
                )
            })?;
        if let Some(error) = app.creation_error {
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

#[cfg(feature = "desktop_winit")]
struct DesktopWindowApp {
    resolved_session: ResolvedSession,
    scene_submit: SceneSubmit,
    session: Option<BoxedDesktopRenderSession>,
    scheduler: FrameScheduler,
    frame_index: u64,
    last_report: Option<FrameReport>,
    window_id: Option<WindowId>,
    creation_error: Option<ZenoError>,
}

#[cfg(feature = "desktop_winit")]
impl ApplicationHandler for DesktopWindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.session.is_some() {
            return;
        }

        match create_desktop_render_session(&self.resolved_session, event_loop) {
            Ok(session) => {
                self.window_id = Some(session.window().id());
                self.scheduler.invalidate_layout();
                session.window().request_redraw();
                self.session = Some(session);
            }
            Err(error) => {
                self.record_window_error("create_session", &error);
                self.creation_error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window_id != Some(window_id) {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if !self.scheduler.has_pending_frame() {
                    return;
                }
                if let Err(error) = self.draw_scene() {
                    self.record_window_error("draw_scene", &error);
                    self.creation_error = Some(error);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(session) = self.session.as_mut() {
                    if let Err(error) = session.resize(size.width, size.height) {
                        self.record_window_error("resize", &error);
                        self.creation_error = Some(error);
                        event_loop.exit();
                    } else {
                        self.scheduler.invalidate_layout();
                        session.window().request_redraw();
                    }
                }
            }
            WindowEvent::Destroyed => {
                self.session = None;
                self.window_id = None;
                event_loop.exit();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.scheduler.has_pending_frame() && let Some(session) = self.session.as_ref() {
            session.window().request_redraw();
        }
    }
}

#[cfg(feature = "desktop_winit")]
impl DesktopWindowApp {
    fn record_window_error(&mut self, op: &'static str, error: &ZenoError) {
        zeno_window_error!(
            "window_runtime_failed",
            error,
            status = "fail",
            backend = ?self.resolved_session.backend.backend_kind,
            frame = self.frame_index,
            frame_stats = self.resolved_session.frame_stats,
            window_id = ?self.window_id,
            caller_op = op,
            "window runtime failed"
        );
    }

    fn draw_scene(&mut self) -> Result<(), ZenoError> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| {
                ZenoError::invalid_configuration(
                    ZenoErrorCode::WindowRendererUnavailable,
                    "shell.window",
                    "draw_scene",
                    "gpu renderer is not available",
                )
            })?;
        let phases = self.scheduler.pending();
        let report = session.submit_scene(&self.scene_submit)?;
        self.frame_index += 1;
        self.last_report = Some(report.clone());
        if self.resolved_session.frame_stats {
            let cache = session.cache_summary();
            if cfg!(debug_assertions) {
                zeno_frame_log!(
                    debug,
                    frame = self.frame_index,
                    backend = ?report.backend,
                    command_count = report.command_count,
                    resource_count = report.resource_count,
                    block_count = report.block_count,
                    patch_upserts = report.patch_upserts,
                    patch_removes = report.patch_removes,
                    layout = phases.needs_layout,
                    paint = phases.needs_paint,
                    present = phases.needs_present,
                    cache = %cache,
                    ?report,
                    "frame stats"
                );
            } else {
                zeno_frame_log!(
                    debug,
                    frame = self.frame_index,
                    backend = ?report.backend,
                    command_count = report.command_count,
                    resource_count = report.resource_count,
                    block_count = report.block_count,
                    patch_upserts = report.patch_upserts,
                    patch_removes = report.patch_removes,
                    layout = phases.needs_layout,
                    paint = phases.needs_paint,
                    present = phases.needs_present,
                    cache = %cache,
                    "frame stats"
                );
            }
        }
        zeno_session_log!(
            trace,
            op = "draw_scene",
            status = "success",
            backend = ?report.backend,
            frame = self.frame_index,
            window_id = ?self.window_id,
            command_count = report.command_count,
            resource_count = report.resource_count,
            "window frame rendered"
        );
        self.scheduler.finish_frame();
        Ok(())
    }
}
