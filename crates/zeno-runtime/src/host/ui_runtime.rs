use std::collections::HashMap;

use zeno_core::{Size, ZenoError, ZenoErrorCode};
use zeno_scene::{CompositorFrame, CompositorFrameStats, DisplayList};
use zeno_text::TextSystem;
use zeno_ui::{
    ComposeEngine, ComposeStats, DirtyReason, ElementId, InteractionRole, InteractionTarget, Node,
    NodeId,
};

use crate::{AppFrame, FramePhases, FrameScheduler, UiEvent};

pub struct UiFrame {
    pub compositor_frame: CompositorFrame<DisplayList>,
    pub phases: FramePhases,
    pub compose_stats: ComposeStats,
}

impl UiFrame {
    #[must_use]
    pub fn display_list(&self) -> &DisplayList {
        &self.compositor_frame.payload
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.compositor_frame.damage.is_full()
    }
}

pub struct UiRuntime<'a> {
    engine: ComposeEngine<'a>,
    scheduler: FrameScheduler,
    root: Option<Node>,
    viewport: Option<Size>,
    focused_element: Option<u64>,
    pointer_press_target: Option<u64>,
    active_touch_targets: HashMap<u64, u64>,
}

impl<'a> UiRuntime<'a> {
    #[must_use]
    pub fn new(text_system: &'a dyn TextSystem) -> Self {
        Self {
            engine: ComposeEngine::new(text_system),
            scheduler: FrameScheduler::new(),
            root: None,
            viewport: None,
            focused_element: None,
            pointer_press_target: None,
            active_touch_targets: HashMap::new(),
        }
    }

    #[must_use]
    pub fn dispatch_events(&mut self, frame: &AppFrame) -> Vec<UiEvent> {
        let mut events = Vec::new();
        self.dispatch_pointer_events(frame, &mut events);
        self.dispatch_touch_events(frame, &mut events);
        self.dispatch_keyboard_events(frame, &mut events);
        events
    }

    pub fn set_root(&mut self, root: Node) {
        if self.root.as_ref() != Some(&root) {
            let had_root = self.root.is_some();
            self.root = Some(root);
            if had_root {
                self.scheduler.invalidate_paint();
            } else {
                self.scheduler.invalidate_layout();
            }
        }
    }

    pub fn resize(&mut self, viewport: Size) {
        if self.viewport != Some(viewport) {
            self.viewport = Some(viewport);
            self.scheduler.invalidate_layout();
        }
    }

    pub fn request_paint(&mut self) {
        self.scheduler.invalidate_paint();
    }

    pub fn request_node_paint(&mut self, node_id: NodeId) {
        self.engine.invalidate_node(node_id, DirtyReason::Paint);
        self.scheduler.invalidate_paint();
    }

    #[must_use]
    pub fn has_pending_frame(&self) -> bool {
        self.scheduler.has_pending_frame()
    }

    pub fn prepare_frame(&mut self) -> Result<Option<UiFrame>, ZenoError> {
        if !self.scheduler.has_pending_frame() {
            return Ok(None);
        }

        let root = self.root.as_ref().ok_or_else(|| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::UiRuntimeRootNotSet,
                "ui.runtime",
                "prepare_frame",
                "ui runtime root is not set",
            )
        })?;
        let viewport = self.viewport.ok_or_else(|| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::UiRuntimeViewportNotConfigured,
                "ui.runtime",
                "prepare_frame",
                "ui runtime viewport is not configured",
            )
        })?;
        let phases = self.scheduler.pending();

        if phases.needs_layout {
            self.engine.invalidate(DirtyReason::Layout);
        } else if phases.needs_paint {
            self.engine.invalidate(DirtyReason::Paint);
        }

        let (compositor_frame, compose_stats) = match self.engine.compose_update(root, viewport) {
            zeno_ui::ComposeUpdate::Full {
                display_list,
                compose_stats,
            } => {
                let generation = display_list.generation;
                (
                    CompositorFrame::full(display_list, generation),
                    compose_stats,
                )
            }
            zeno_ui::ComposeUpdate::Delta {
                damage,
                patch_upserts,
                patch_removes,
                display_list,
                compose_stats,
            } => {
                let generation = display_list.generation;
                (
                    CompositorFrame::new(
                        display_list,
                        damage,
                        generation,
                        CompositorFrameStats {
                            patch_upserts,
                            patch_removes,
                        },
                    ),
                    compose_stats,
                )
            }
        };
        let frame = UiFrame {
            compositor_frame,
            phases,
            compose_stats,
        };
        self.reconcile_focus();
        self.scheduler.finish_frame();
        Ok(Some(frame))
    }

    pub fn request_node_layout(&mut self, node_id: NodeId) {
        self.engine.invalidate_node(node_id, DirtyReason::Layout);
        self.scheduler.invalidate_layout();
    }

    fn dispatch_pointer_events(&mut self, frame: &AppFrame, out: &mut Vec<UiEvent>) {
        let current_hit_target = frame
            .pointer
            .position
            .and_then(|position| self.engine.hit_test(position));
        let press_hit_target = frame
            .pointer
            .press_position
            .and_then(|position| self.engine.hit_test(position));
        let release_hit_target = frame
            .pointer
            .release_position
            .and_then(|position| self.engine.hit_test(position));
        let hit_target = if frame.pointer.just_released {
            release_hit_target.or(current_hit_target)
        } else if frame.pointer.just_pressed {
            press_hit_target.or(current_hit_target)
        } else {
            current_hit_target
        };
        if frame.pointer.just_pressed {
            self.pointer_press_target = hit_target.map(|target| target.element_id.0);
            if let Some(target) = hit_target
                && target.interaction.is_focusable()
            {
                self.update_focus(Some(target.node_id), out);
            }
        }
        if frame.pointer.just_released {
            let pressed_target = self.pointer_press_target.take();
            if let (Some(start), Some(end)) = (pressed_target, hit_target)
                && start == end.element_id.0
            {
                out.extend(self.activate_target(end));
            }
        }
    }

    fn dispatch_touch_events(&mut self, frame: &AppFrame, out: &mut Vec<UiEvent>) {
        for touch in &frame.touches {
            let hit_target = self.engine.hit_test(touch.position);
            match touch.phase {
                zeno_platform::event::TouchPhase::Started => {
                    if let Some(target) = hit_target {
                        self.active_touch_targets
                            .insert(touch.id, target.element_id.0);
                        if target.interaction.is_focusable() {
                            self.update_focus(Some(target.node_id), out);
                        }
                    }
                }
                zeno_platform::event::TouchPhase::Ended => {
                    let started = self.active_touch_targets.remove(&touch.id);
                    if let (Some(start), Some(end)) = (started, hit_target)
                        && start == end.element_id.0
                    {
                        out.extend(self.activate_target(end));
                    }
                }
                zeno_platform::event::TouchPhase::Cancelled => {
                    self.active_touch_targets.remove(&touch.id);
                }
                zeno_platform::event::TouchPhase::Moved => {}
            }
        }
    }

    fn dispatch_keyboard_events(&mut self, frame: &AppFrame, out: &mut Vec<UiEvent>) {
        for key_event in &frame.keyboard {
            if matches!(key_event.state, zeno_platform::event::KeyState::Pressed)
                && !key_event.repeat
                && matches!(key_event.key, zeno_platform::event::Key::Tab)
            {
                self.advance_focus(key_event.modifiers.shift, out);
                continue;
            }

            let Some(target) = self.focused_target() else {
                continue;
            };
            let Some(action_id) = target.interaction.action else {
                continue;
            };
            out.push(UiEvent::KeyInput {
                action_id,
                key: key_event.key.clone(),
                state: key_event.state,
                repeat: key_event.repeat,
                modifiers: key_event.modifiers,
            });
            if matches!(key_event.state, zeno_platform::event::KeyState::Pressed)
                && !key_event.repeat
                && matches!(
                    key_event.key,
                    zeno_platform::event::Key::Enter | zeno_platform::event::Key::Space
                )
            {
                out.extend(self.activate_target(target));
            }
        }

        let Some(target) = self.focused_target() else {
            return;
        };
        if !target.interaction.accepts_text_input {
            return;
        }
        let Some(action_id) = target.interaction.action else {
            return;
        };
        for text_event in &frame.text_input {
            out.push(UiEvent::TextInput {
                action_id,
                text: text_event.text.clone(),
            });
        }
    }

    fn activate_target(&self, target: InteractionTarget) -> Vec<UiEvent> {
        if !target.interaction.enabled {
            return Vec::new();
        }
        let Some(action_id) = target.interaction.action else {
            return Vec::new();
        };
        match target.interaction.role {
            Some(
                InteractionRole::Checkbox | InteractionRole::Switch | InteractionRole::ToggleButton,
            ) => {
                vec![UiEvent::ToggleChanged {
                    action_id,
                    checked: !target.interaction.checked.unwrap_or(false),
                }]
            }
            _ => vec![UiEvent::Click { action_id }],
        }
    }

    fn focused_target(&self) -> Option<InteractionTarget> {
        self.focused_element.and_then(|element_id| {
            self.engine
                .interaction_target_by_element(ElementId(element_id))
        })
    }

    fn update_focus(&mut self, next_focus: Option<NodeId>, out: &mut Vec<UiEvent>) {
        let next_element = next_focus
            .and_then(|node_id| self.engine.interaction_target(node_id))
            .map(|target| target.element_id.0);
        if self.focused_element == next_element {
            return;
        }
        if let Some(previous) = self.focused_element
            && let Some(target) = self
                .engine
                .interaction_target_by_element(ElementId(previous))
            && let Some(action_id) = target.interaction.action
        {
            out.push(UiEvent::FocusChanged {
                action_id,
                focused: false,
            });
        }
        self.focused_element = next_element;
        if let Some(current) = self.focused_element
            && let Some(target) = self
                .engine
                .interaction_target_by_element(ElementId(current))
            && let Some(action_id) = target.interaction.action
        {
            out.push(UiEvent::FocusChanged {
                action_id,
                focused: true,
            });
        }
    }

    fn advance_focus(&mut self, reverse: bool, out: &mut Vec<UiEvent>) {
        let targets = self.engine.focusable_targets();
        if targets.is_empty() {
            return;
        }
        let current_index = self.focused_element.and_then(|focused| {
            targets
                .iter()
                .position(|target| target.element_id.0 == focused)
        });
        let next_index = match current_index {
            Some(index) if reverse => index.checked_sub(1).unwrap_or(targets.len() - 1),
            Some(index) => (index + 1) % targets.len(),
            None if reverse => targets.len() - 1,
            None => 0,
        };
        self.update_focus(Some(targets[next_index].node_id), out);
    }

    fn reconcile_focus(&mut self) {
        if let Some(focused) = self.focused_element
            && self
                .engine
                .interaction_target_by_element(ElementId(focused))
                .is_none()
        {
            self.focused_element = None;
        }
        if let Some(pressed) = self.pointer_press_target
            && self
                .engine
                .interaction_target_by_element(ElementId(pressed))
                .is_none()
        {
            self.pointer_press_target = None;
        }
        self.active_touch_targets.retain(|_, target| {
            self.engine
                .interaction_target_by_element(ElementId(*target))
                .is_some()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use zeno_core::{Backend, Platform, Point};
    use zeno_platform::event::{Key, KeyState, KeyboardEvent, KeyboardModifiers, TextInputEvent};
    use zeno_scene::FrameReport;
    use zeno_text::FallbackTextSystem;
    use zeno_ui::{ActionId, NodeKind, SpacerNode};

    fn make_frame(
        pointer: crate::PointerState,
        keyboard: Vec<KeyboardEvent>,
        text_input: Vec<TextInputEvent>,
    ) -> AppFrame {
        AppFrame {
            frame_index: 0,
            elapsed: Duration::default(),
            delta: Duration::default(),
            size: Size::new(200.0, 120.0),
            platform: Platform::Linux,
            backend: Backend::Skia,
            last_report: Some(FrameReport {
                backend: Backend::Skia,
                command_count: 0,
                resource_count: 0,
                block_count: 0,
                display_item_count: 0,
                stacking_context_count: 0,
                damage_rect_count: 0,
                damage_full: true,
                dirty_tile_count: 0,
                cached_tile_count: 0,
                reraster_tile_count: 0,
                raster_batch_tile_count: 0,
                composite_tile_count: 0,
                compositor_layer_count: 0,
                offscreen_layer_count: 0,
                tile_content_handle_count: 0,
                compositor_task_count: 0,
                compositor_queue_depth: 0,
                compositor_dropped_frame_count: 0,
                compositor_processed_frame_count: 0,
                released_tile_resource_count: 0,
                evicted_tile_resource_count: 0,
                budget_evicted_tile_resource_count: 0,
                age_evicted_tile_resource_count: 0,
                descriptor_limit_evicted_tile_resource_count: 0,
                reused_tile_resource_count: 0,
                reusable_tile_resource_count: 0,
                reusable_tile_resource_bytes: 0,
                tile_resource_reuse_budget_bytes: 0,
                compositor_worker_threaded: false,
                compositor_worker_alive: false,
                composite_executed_layer_count: 0,
                composite_executed_tile_count: 0,
                composite_offscreen_step_count: 0,
                surface_id: "test".into(),
            }),
            pointer,
            touches: Vec::new(),
            keyboard,
            text_input,
        }
    }

    fn interactive_spacer(id: u64) -> Node {
        Node::new(
            NodeId(id),
            NodeKind::Spacer(SpacerNode {
                width: 40.0,
                height: 40.0,
            }),
        )
        .fixed_size(40.0, 40.0)
    }

    #[test]
    fn pointer_click_dispatches_toggle_changed() {
        let mut runtime = UiRuntime::new(&FallbackTextSystem);
        runtime.resize(Size::new(200.0, 120.0));
        runtime.set_root(
            interactive_spacer(1)
                .modifier(zeno_ui::Modifier::InteractionRole(
                    InteractionRole::Checkbox,
                ))
                .action(ActionId(11))
                .modifier(zeno_ui::Modifier::Checked(false))
                .focusable(),
        );
        runtime
            .prepare_frame()
            .expect("prepare frame")
            .expect("ui frame");

        let press_events = runtime.dispatch_events(&make_frame(
            crate::PointerState {
                position: Some(Point::new(10.0, 10.0)),
                press_position: Some(Point::new(10.0, 10.0)),
                release_position: None,
                pressed: true,
                just_pressed: true,
                just_released: false,
            },
            Vec::new(),
            Vec::new(),
        ));
        assert_eq!(
            press_events,
            vec![UiEvent::FocusChanged {
                action_id: ActionId(11),
                focused: true,
            }]
        );

        let release_events = runtime.dispatch_events(&make_frame(
            crate::PointerState {
                position: Some(Point::new(10.0, 10.0)),
                press_position: None,
                release_position: Some(Point::new(10.0, 10.0)),
                pressed: false,
                just_pressed: false,
                just_released: true,
            },
            Vec::new(),
            Vec::new(),
        ));
        assert_eq!(
            release_events,
            vec![UiEvent::ToggleChanged {
                action_id: ActionId(11),
                checked: true,
            }]
        );
    }

    #[test]
    fn disabled_target_does_not_dispatch_toggle_changed() {
        let mut runtime = UiRuntime::new(&FallbackTextSystem);
        runtime.resize(Size::new(200.0, 120.0));
        runtime.set_root(
            interactive_spacer(3)
                .modifier(zeno_ui::Modifier::InteractionRole(
                    InteractionRole::Checkbox,
                ))
                .action(ActionId(31))
                .modifier(zeno_ui::Modifier::Checked(false))
                .modifier(zeno_ui::Modifier::Enabled(false))
                .focusable(),
        );
        runtime
            .prepare_frame()
            .expect("prepare frame")
            .expect("ui frame");

        let press_events = runtime.dispatch_events(&make_frame(
            crate::PointerState {
                position: Some(Point::new(10.0, 10.0)),
                press_position: Some(Point::new(10.0, 10.0)),
                release_position: None,
                pressed: true,
                just_pressed: true,
                just_released: false,
            },
            Vec::new(),
            Vec::new(),
        ));
        assert!(press_events.is_empty());

        let release_events = runtime.dispatch_events(&make_frame(
            crate::PointerState {
                position: Some(Point::new(10.0, 10.0)),
                press_position: None,
                release_position: Some(Point::new(10.0, 10.0)),
                pressed: false,
                just_pressed: false,
                just_released: true,
            },
            Vec::new(),
            Vec::new(),
        ));
        assert!(release_events.is_empty());
    }

    #[test]
    fn action_backed_target_survives_node_id_changes_between_press_and_release() {
        let mut runtime = UiRuntime::new(&FallbackTextSystem);
        runtime.resize(Size::new(200.0, 120.0));
        runtime.set_root(
            interactive_spacer(10)
                .modifier(zeno_ui::Modifier::InteractionRole(InteractionRole::Switch))
                .action(ActionId(41))
                .modifier(zeno_ui::Modifier::Checked(false))
                .focusable(),
        );
        runtime
            .prepare_frame()
            .expect("prepare frame")
            .expect("ui frame");

        let press_events = runtime.dispatch_events(&make_frame(
            crate::PointerState {
                position: Some(Point::new(10.0, 10.0)),
                press_position: Some(Point::new(10.0, 10.0)),
                release_position: None,
                pressed: true,
                just_pressed: true,
                just_released: false,
            },
            Vec::new(),
            Vec::new(),
        ));
        assert_eq!(
            press_events,
            vec![UiEvent::FocusChanged {
                action_id: ActionId(41),
                focused: true,
            }]
        );

        runtime.set_root(
            interactive_spacer(11)
                .modifier(zeno_ui::Modifier::InteractionRole(InteractionRole::Switch))
                .action(ActionId(41))
                .modifier(zeno_ui::Modifier::Checked(false))
                .focusable(),
        );
        runtime
            .prepare_frame()
            .expect("prepare frame")
            .expect("ui frame");

        let release_events = runtime.dispatch_events(&make_frame(
            crate::PointerState {
                position: Some(Point::new(10.0, 10.0)),
                press_position: None,
                release_position: Some(Point::new(10.0, 10.0)),
                pressed: false,
                just_pressed: false,
                just_released: true,
            },
            Vec::new(),
            Vec::new(),
        ));
        assert_eq!(
            release_events,
            vec![UiEvent::ToggleChanged {
                action_id: ActionId(41),
                checked: true,
            }]
        );
    }

    #[test]
    fn release_uses_release_position_instead_of_latest_pointer_position() {
        let mut runtime = UiRuntime::new(&FallbackTextSystem);
        runtime.resize(Size::new(200.0, 120.0));
        runtime.set_root(
            interactive_spacer(12)
                .modifier(zeno_ui::Modifier::InteractionRole(
                    InteractionRole::Checkbox,
                ))
                .action(ActionId(51))
                .modifier(zeno_ui::Modifier::Checked(false))
                .focusable(),
        );
        runtime
            .prepare_frame()
            .expect("prepare frame")
            .expect("ui frame");

        let _press_events = runtime.dispatch_events(&make_frame(
            crate::PointerState {
                position: Some(Point::new(10.0, 10.0)),
                press_position: Some(Point::new(10.0, 10.0)),
                release_position: None,
                pressed: true,
                just_pressed: true,
                just_released: false,
            },
            Vec::new(),
            Vec::new(),
        ));

        let release_events = runtime.dispatch_events(&make_frame(
            crate::PointerState {
                position: Some(Point::new(180.0, 100.0)),
                press_position: None,
                release_position: Some(Point::new(10.0, 10.0)),
                pressed: false,
                just_pressed: false,
                just_released: true,
            },
            Vec::new(),
            Vec::new(),
        ));
        assert_eq!(
            release_events,
            vec![UiEvent::ToggleChanged {
                action_id: ActionId(51),
                checked: true,
            }]
        );
    }

    #[test]
    fn tab_focus_and_text_input_dispatch_keyboard_events() {
        let mut runtime = UiRuntime::new(&FallbackTextSystem);
        runtime.resize(Size::new(200.0, 120.0));
        runtime.set_root(
            interactive_spacer(2)
                .action(ActionId(21))
                .accept_text_input(),
        );
        runtime
            .prepare_frame()
            .expect("prepare frame")
            .expect("ui frame");

        let focus_events = runtime.dispatch_events(&make_frame(
            crate::PointerState::default(),
            vec![KeyboardEvent {
                key: Key::Tab,
                state: KeyState::Pressed,
                repeat: false,
                modifiers: KeyboardModifiers::default(),
            }],
            Vec::new(),
        ));
        assert_eq!(
            focus_events,
            vec![UiEvent::FocusChanged {
                action_id: ActionId(21),
                focused: true,
            }]
        );

        let input_events = runtime.dispatch_events(&make_frame(
            crate::PointerState::default(),
            vec![KeyboardEvent {
                key: Key::Character("a".into()),
                state: KeyState::Pressed,
                repeat: false,
                modifiers: KeyboardModifiers::default(),
            }],
            vec![TextInputEvent { text: "a".into() }],
        ));
        assert_eq!(
            input_events,
            vec![
                UiEvent::KeyInput {
                    action_id: ActionId(21),
                    key: Key::Character("a".into()),
                    state: KeyState::Pressed,
                    repeat: false,
                    modifiers: KeyboardModifiers::default(),
                },
                UiEvent::TextInput {
                    action_id: ActionId(21),
                    text: "a".into(),
                },
            ]
        );
    }
}
