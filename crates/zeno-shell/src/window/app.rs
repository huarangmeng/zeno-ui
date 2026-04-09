use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;
use zeno_core::{ZenoError, ZenoErrorCode, zeno_frame_log, zeno_session_log, zeno_window_error};
use zeno_graphics::{FrameReport, SceneSubmit};
use zeno_runtime::{FrameScheduler, ResolvedSession};

use crate::desktop_session::{BoxedDesktopRenderSession, create_desktop_render_session};

pub(super) struct DesktopWindowApp {
    resolved_session: ResolvedSession,
    scene_submit: SceneSubmit,
    session: Option<BoxedDesktopRenderSession>,
    scheduler: FrameScheduler,
    frame_index: u64,
    last_report: Option<FrameReport>,
    window_id: Option<WindowId>,
    creation_error: Option<ZenoError>,
}

impl DesktopWindowApp {
    pub(super) fn new(resolved_session: ResolvedSession, scene_submit: SceneSubmit) -> Self {
        Self {
            resolved_session,
            scene_submit,
            session: None,
            scheduler: FrameScheduler::new(),
            frame_index: 0,
            last_report: None,
            window_id: None,
            creation_error: None,
        }
    }

    pub(super) fn into_creation_error(self) -> Option<ZenoError> {
        self.creation_error
    }

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
        let session = self.session.as_mut().ok_or_else(|| {
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
        if self.scheduler.has_pending_frame()
            && let Some(session) = self.session.as_ref()
        {
            session.window().request_redraw();
        }
    }
}
