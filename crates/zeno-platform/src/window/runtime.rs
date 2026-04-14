use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, MouseButton, TouchPhase as WinitTouchPhase, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
use winit::window::WindowId;
use zeno_core::{ZenoError, ZenoErrorCode, zeno_frame_log, zeno_session_log, zeno_window_error};
use zeno_scene::{CompositorFrame, DisplayList, FrameReport};

use crate::event::{
    Key, KeyState, KeyboardEvent, KeyboardModifiers, PointerState, TextInputEvent, TouchEvent,
    TouchPhase,
};
use crate::NativeSurface;
use crate::desktop_session::{BoxedDesktopRenderSession, create_desktop_render_session};
use crate::session::ResolvedSession;
use crate::window::{AnimatedFrameContext, BoxedAnimatedSceneCallback, FrameRequest};

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
    touch_events: Vec<TouchEvent>,
    keyboard_events: Vec<KeyboardEvent>,
    text_input_events: Vec<TextInputEvent>,
    modifiers: KeyboardModifiers,
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
            touch_events: Vec::new(),
            keyboard_events: Vec::new(),
            text_input_events: Vec::new(),
            modifiers: KeyboardModifiers::default(),
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
            touch_events: Vec::new(),
            keyboard_events: Vec::new(),
            text_input_events: Vec::new(),
            modifiers: KeyboardModifiers::default(),
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
        let touches = self.touch_events.clone();
        let keyboard = self.keyboard_events.clone();
        let text_input = self.text_input_events.clone();
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
                    session.submit_compositor_frame(&CompositorFrame::full(
                        display_list.clone(),
                        display_list.generation,
                    ))?,
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
                            touches,
                            keyboard,
                            text_input,
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
        self.pointer_state.press_position = None;
        self.pointer_state.release_position = None;
        self.touch_events.clear();
        self.keyboard_events.clear();
        self.text_input_events.clear();
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
                    damage_rect_count = report.damage_rect_count,
                    damage_full = report.damage_full,
                    dirty_tile_count = report.dirty_tile_count,
                    cached_tile_count = report.cached_tile_count,
                    reraster_tile_count = report.reraster_tile_count,
                    raster_batch_tile_count = report.raster_batch_tile_count,
                    composite_tile_count = report.composite_tile_count,
                    compositor_layer_count = report.compositor_layer_count,
                    offscreen_layer_count = report.offscreen_layer_count,
                    tile_content_handle_count = report.tile_content_handle_count,
                    compositor_task_count = report.compositor_task_count,
                    compositor_queue_depth = report.compositor_queue_depth,
                    compositor_dropped_frame_count = report.compositor_dropped_frame_count,
                    compositor_processed_frame_count = report.compositor_processed_frame_count,
                    released_tile_resource_count = report.released_tile_resource_count,
                    evicted_tile_resource_count = report.evicted_tile_resource_count,
                    budget_evicted_tile_resource_count = report.budget_evicted_tile_resource_count,
                    age_evicted_tile_resource_count = report.age_evicted_tile_resource_count,
                    descriptor_limit_evicted_tile_resource_count = report.descriptor_limit_evicted_tile_resource_count,
                    reused_tile_resource_count = report.reused_tile_resource_count,
                    reusable_tile_resource_count = report.reusable_tile_resource_count,
                    reusable_tile_resource_bytes = report.reusable_tile_resource_bytes,
                    tile_resource_reuse_budget_bytes = report.tile_resource_reuse_budget_bytes,
                    compositor_worker_threaded = report.compositor_worker_threaded,
                    compositor_worker_alive = report.compositor_worker_alive,
                    composite_executed_layer_count = report.composite_executed_layer_count,
                    composite_executed_tile_count = report.composite_executed_tile_count,
                    composite_offscreen_step_count = report.composite_offscreen_step_count,
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
                    damage_rect_count = report.damage_rect_count,
                    damage_full = report.damage_full,
                    dirty_tile_count = report.dirty_tile_count,
                    cached_tile_count = report.cached_tile_count,
                    reraster_tile_count = report.reraster_tile_count,
                    raster_batch_tile_count = report.raster_batch_tile_count,
                    composite_tile_count = report.composite_tile_count,
                    compositor_layer_count = report.compositor_layer_count,
                    offscreen_layer_count = report.offscreen_layer_count,
                    tile_content_handle_count = report.tile_content_handle_count,
                    compositor_task_count = report.compositor_task_count,
                    compositor_queue_depth = report.compositor_queue_depth,
                    compositor_dropped_frame_count = report.compositor_dropped_frame_count,
                    compositor_processed_frame_count = report.compositor_processed_frame_count,
                    released_tile_resource_count = report.released_tile_resource_count,
                    evicted_tile_resource_count = report.evicted_tile_resource_count,
                    budget_evicted_tile_resource_count = report.budget_evicted_tile_resource_count,
                    age_evicted_tile_resource_count = report.age_evicted_tile_resource_count,
                    descriptor_limit_evicted_tile_resource_count = report.descriptor_limit_evicted_tile_resource_count,
                    reused_tile_resource_count = report.reused_tile_resource_count,
                    reusable_tile_resource_count = report.reusable_tile_resource_count,
                    reusable_tile_resource_bytes = report.reusable_tile_resource_bytes,
                    tile_resource_reuse_budget_bytes = report.tile_resource_reuse_budget_bytes,
                    compositor_worker_threaded = report.compositor_worker_threaded,
                    compositor_worker_alive = report.compositor_worker_alive,
                    composite_executed_layer_count = report.composite_executed_layer_count,
                    composite_executed_tile_count = report.composite_executed_tile_count,
                    composite_offscreen_step_count = report.composite_offscreen_step_count,
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
            dirty_tile_count = report.dirty_tile_count,
            cached_tile_count = report.cached_tile_count,
            reraster_tile_count = report.reraster_tile_count,
            raster_batch_tile_count = report.raster_batch_tile_count,
            composite_tile_count = report.composite_tile_count,
            compositor_layer_count = report.compositor_layer_count,
            offscreen_layer_count = report.offscreen_layer_count,
            tile_content_handle_count = report.tile_content_handle_count,
            compositor_task_count = report.compositor_task_count,
            compositor_queue_depth = report.compositor_queue_depth,
            compositor_dropped_frame_count = report.compositor_dropped_frame_count,
            compositor_processed_frame_count = report.compositor_processed_frame_count,
            released_tile_resource_count = report.released_tile_resource_count,
            evicted_tile_resource_count = report.evicted_tile_resource_count,
            budget_evicted_tile_resource_count = report.budget_evicted_tile_resource_count,
            age_evicted_tile_resource_count = report.age_evicted_tile_resource_count,
            descriptor_limit_evicted_tile_resource_count = report.descriptor_limit_evicted_tile_resource_count,
            reused_tile_resource_count = report.reused_tile_resource_count,
            reusable_tile_resource_count = report.reusable_tile_resource_count,
            reusable_tile_resource_bytes = report.reusable_tile_resource_bytes,
            tile_resource_reuse_budget_bytes = report.tile_resource_reuse_budget_bytes,
            compositor_worker_threaded = report.compositor_worker_threaded,
            compositor_worker_alive = report.compositor_worker_alive,
            composite_executed_layer_count = report.composite_executed_layer_count,
            composite_executed_tile_count = report.composite_executed_tile_count,
            composite_offscreen_step_count = report.composite_offscreen_step_count,
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
                        self.pointer_state.press_position = self.pointer_state.position;
                    }
                    ElementState::Released if self.pointer_state.pressed => {
                        self.pointer_state.pressed = false;
                        self.pointer_state.just_released = true;
                        self.pointer_state.release_position = self.pointer_state.position;
                    }
                    _ => {}
                }
                if self.is_animated()
                    && let Some(session) = self.session.as_ref()
                {
                    session.window().request_redraw();
                }
            }
            WindowEvent::Touch(touch) => {
                self.touch_events.push(TouchEvent {
                    id: touch.id,
                    phase: match touch.phase {
                        WinitTouchPhase::Started => TouchPhase::Started,
                        WinitTouchPhase::Moved => TouchPhase::Moved,
                        WinitTouchPhase::Ended => TouchPhase::Ended,
                        WinitTouchPhase::Cancelled => TouchPhase::Cancelled,
                    },
                    position: zeno_core::Point::new(
                        touch.location.x as f32,
                        touch.location.y as f32,
                    ),
                    force: touch.force.map(|force| match force {
                        winit::event::Force::Calibrated {
                            force,
                            max_possible_force,
                            ..
                        } => (force / max_possible_force.max(f64::EPSILON)) as f32,
                        winit::event::Force::Normalized(force) => force as f32,
                    }),
                });
                if self.is_animated()
                    && let Some(session) = self.session.as_ref()
                {
                    session.window().request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers_state_to_keyboard_modifiers(modifiers.state());
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.keyboard_events.push(KeyboardEvent {
                    key: map_winit_key(&event.logical_key),
                    state: match event.state {
                        ElementState::Pressed => KeyState::Pressed,
                        ElementState::Released => KeyState::Released,
                    },
                    repeat: event.repeat,
                    modifiers: self.modifiers,
                });
                if self.is_animated()
                    && let Some(session) = self.session.as_ref()
                {
                    session.window().request_redraw();
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) if !text.is_empty() => {
                self.text_input_events.push(TextInputEvent { text });
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

fn modifiers_state_to_keyboard_modifiers(modifiers: ModifiersState) -> KeyboardModifiers {
    KeyboardModifiers {
        shift: modifiers.shift_key(),
        control: modifiers.control_key(),
        alt: modifiers.alt_key(),
        meta: modifiers.super_key(),
    }
}

fn map_winit_key(key: &WinitKey) -> Key {
    match key {
        WinitKey::Character(text) => {
            if text == " " {
                Key::Space
            } else {
                Key::Character(text.to_string())
            }
        }
        WinitKey::Named(named) => match named {
            NamedKey::Enter => Key::Enter,
            NamedKey::Space => Key::Space,
            NamedKey::Tab => Key::Tab,
            NamedKey::Escape => Key::Escape,
            NamedKey::Backspace => Key::Backspace,
            NamedKey::Delete => Key::Delete,
            NamedKey::ArrowUp => Key::ArrowUp,
            NamedKey::ArrowDown => Key::ArrowDown,
            NamedKey::ArrowLeft => Key::ArrowLeft,
            NamedKey::ArrowRight => Key::ArrowRight,
            NamedKey::Home => Key::Home,
            NamedKey::End => Key::End,
            NamedKey::PageUp => Key::PageUp,
            NamedKey::PageDown => Key::PageDown,
            other => Key::Unknown(format!("{other:?}")),
        },
        other => Key::Unknown(format!("{other:?}")),
    }
}
