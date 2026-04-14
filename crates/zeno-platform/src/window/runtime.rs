use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;
use zeno_core::{ZenoError, ZenoErrorCode, zeno_frame_log, zeno_session_log, zeno_window_error};
use zeno_scene::{DisplayList, FrameReport};

use crate::NativeSurface;
use crate::desktop_session::{BoxedDesktopRenderSession, create_desktop_render_session};
use crate::session::ResolvedSession;
use crate::window::{AnimatedFrameContext, BoxedAnimatedSceneCallback, FrameRequest, PointerState};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct PendingFramePhases {
    needs_layout: bool,
    needs_paint: bool,
    needs_present: bool,
}

impl PendingFramePhases {
    fn invalidate_layout(&mut self) {
        self.needs_layout = true;
        self.needs_paint = true;
        self.needs_present = true;
    }

    fn finish_frame(&mut self) {
        *self = Self::default();
    }

    fn has_pending_frame(self) -> bool {
        self.needs_layout || self.needs_paint || self.needs_present
    }
}

enum SceneDriver {
    Static(DisplayList),
    Animated {
        callback: BoxedAnimatedSceneCallback,
        started_at: Instant,
        last_frame_at: Option<Instant>,
    },
}

pub(super) struct DesktopWindowApp {
    resolved_session: ResolvedSession,
    native_surface: NativeSurface,
    scene_driver: SceneDriver,
    session: Option<BoxedDesktopRenderSession>,
    phases: PendingFramePhases,
    frame_index: u64,
    last_report: Option<FrameReport>,
    window_id: Option<WindowId>,
    creation_error: Option<ZenoError>,
    pointer_state: PointerState,
    next_frame_request: FrameRequest,
    next_frame_deadline: Option<Instant>,
}

impl DesktopWindowApp {
    pub(super) fn new(
        resolved_session: ResolvedSession,
        native_surface: NativeSurface,
        display_list: DisplayList,
    ) -> Self {
        Self {
            resolved_session,
            native_surface,
            scene_driver: SceneDriver::Static(display_list),
            session: None,
            phases: PendingFramePhases::default(),
            frame_index: 0,
            last_report: None,
            window_id: None,
            creation_error: None,
            pointer_state: PointerState::default(),
            next_frame_request: FrameRequest::Wait,
            next_frame_deadline: None,
        }
    }

    pub(super) fn new_animated(
        resolved_session: ResolvedSession,
        native_surface: NativeSurface,
        callback: BoxedAnimatedSceneCallback,
    ) -> Self {
        Self {
            resolved_session,
            native_surface,
            scene_driver: SceneDriver::Animated {
                callback,
                started_at: Instant::now(),
                last_frame_at: None,
            },
            session: None,
            phases: PendingFramePhases::default(),
            frame_index: 0,
            last_report: None,
            window_id: None,
            creation_error: None,
            pointer_state: PointerState::default(),
            next_frame_request: FrameRequest::NextFrame,
            next_frame_deadline: None,
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

    fn is_animated(&self) -> bool {
        matches!(self.scene_driver, SceneDriver::Animated { .. })
    }

    fn draw_scene(&mut self) -> Result<(), ZenoError> {
        let window_size = self
            .session
            .as_ref()
            .map(|session| session.window().inner_size())
            .ok_or_else(|| {
                ZenoError::invalid_configuration(
                    ZenoErrorCode::WindowRendererUnavailable,
                    "shell.window",
                    "draw_scene",
                    "gpu renderer is not available",
                )
            })?;
        let frame_index = self.frame_index;
        let backend = self.resolved_session.backend.backend_kind;
        let last_report = self.last_report.clone();
        let pointer = self.pointer_state.clone();
        let phases = self.phases;
        let mut session = self.session.take().ok_or_else(|| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::WindowRendererUnavailable,
                "shell.window",
                "draw_scene",
                "gpu renderer is not available",
            )
        })?;
        let (frame_request, report, cache) = {
            let (frame_request, report) = match &mut self.scene_driver {
                SceneDriver::Static(display_list) => (
                    FrameRequest::Wait,
                    session.submit_display_list(display_list, None, 0, 0)?,
                ),
                SceneDriver::Animated {
                    callback,
                    started_at,
                    last_frame_at,
                } => {
                    let now = Instant::now();
                    let delta = last_frame_at
                        .map(|timestamp| now.saturating_duration_since(timestamp))
                        .unwrap_or_default();
                    *last_frame_at = Some(now);
                    let output = callback(
                        AnimatedFrameContext {
                            frame_index,
                            elapsed: now.saturating_duration_since(*started_at),
                            delta,
                            size: zeno_core::Size::new(
                                window_size.width as f32,
                                window_size.height as f32,
                            ),
                            backend,
                            last_report,
                            pointer,
                        },
                        session.as_mut(),
                    )?;
                    let report = output.report.ok_or_else(|| {
                        ZenoError::invalid_configuration(
                            ZenoErrorCode::WindowRunAppFailed,
                            "shell.window",
                            "animated_callback",
                            "animated callback must return a submitted frame report",
                        )
                    })?;
                    (output.frame_request, report)
                }
            };
            let cache = if self.resolved_session.frame_stats {
                Some(session.cache_summary())
            } else {
                None
            };
            (frame_request, report, cache)
        };
        self.session = Some(session);
        self.next_frame_request = frame_request;
        self.next_frame_deadline = match self.next_frame_request {
            FrameRequest::After(duration) => Some(Instant::now() + duration),
            _ => None,
        };
        self.frame_index += 1;
        self.last_report = Some(report.clone());
        self.pointer_state.just_pressed = false;
        self.pointer_state.just_released = false;
        if self.resolved_session.frame_stats {
            let cache = cache.expect("cache summary should exist when frame stats are enabled");
            if cfg!(debug_assertions) {
                zeno_frame_log!(
                    debug,
                    frame = self.frame_index,
                    backend = ?report.backend,
                    command_count = report.command_count,
                    resource_count = report.resource_count,
                    block_count = report.block_count,
                    display_item_count = report.display_item_count,
                    stacking_context_count = report.stacking_context_count,
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
            display_item_count = report.display_item_count,
            stacking_context_count = report.stacking_context_count,
            "window frame rendered"
        );
        self.phases.finish_frame();
        Ok(())
    }
}

impl ApplicationHandler for DesktopWindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.session.is_some() {
            return;
        }

        match create_desktop_render_session(
            &self.resolved_session,
            &self.native_surface,
            event_loop,
        ) {
            Ok(session) => {
                self.window_id = Some(session.window().id());
                self.phases.invalidate_layout();
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
                if !self.is_animated() && !self.phases.has_pending_frame() {
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
                        self.phases.invalidate_layout();
                        session.window().request_redraw();
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer_state.position =
                    Some(zeno_core::Point::new(position.x as f32, position.y as f32));
                if self.is_animated()
                    && let Some(session) = self.session.as_ref()
                {
                    session.window().request_redraw();
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer_state.position = None;
                if self.is_animated()
                    && let Some(session) = self.session.as_ref()
                {
                    session.window().request_redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } if button == MouseButton::Left => {
                match state {
                    ElementState::Pressed if !self.pointer_state.pressed => {
                        self.pointer_state.pressed = true;
                        self.pointer_state.just_pressed = true;
                    }
                    ElementState::Released if self.pointer_state.pressed => {
                        self.pointer_state.pressed = false;
                        self.pointer_state.just_released = true;
                    }
                    _ => {}
                }
                if self.is_animated()
                    && let Some(session) = self.session.as_ref()
                {
                    session.window().request_redraw();
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(session) = self.session.as_ref() {
            if self.is_animated() {
                match self.next_frame_request {
                    FrameRequest::Wait => {
                        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
                    }
                    FrameRequest::NextFrame => {
                        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
                        session.window().request_redraw();
                    }
                    FrameRequest::After(duration) => {
                        let deadline = self
                            .next_frame_deadline
                            .unwrap_or_else(|| Instant::now() + duration);
                        event_loop
                            .set_control_flow(winit::event_loop::ControlFlow::WaitUntil(deadline));
                        if Instant::now() >= deadline {
                            session.window().request_redraw();
                        }
                    }
                }
            } else if self.phases.has_pending_frame() {
                session.window().request_redraw();
            }
        }
    }
}
