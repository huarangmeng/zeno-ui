use std::time::Duration;

use zeno_core::{Backend, Platform, Point, Size};
use zeno_platform::event::{
    Key, KeyState, KeyboardEvent, KeyboardModifiers, TextInputEvent, TouchEvent,
};
use zeno_scene::FrameReport;
use zeno_ui::{ActionId, Node};

#[derive(Debug, Clone, PartialEq)]
pub struct PointerState {
    pub position: Option<Point>,
    pub press_position: Option<Point>,
    pub release_position: Option<Point>,
    pub pressed: bool,
    pub just_pressed: bool,
    pub just_released: bool,
}

impl Default for PointerState {
    fn default() -> Self {
        Self {
            position: None,
            press_position: None,
            release_position: None,
            pressed: false,
            just_pressed: false,
            just_released: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppFrame {
    pub frame_index: u64,
    pub elapsed: Duration,
    pub delta: Duration,
    pub size: Size,
    pub platform: Platform,
    pub backend: Backend,
    pub last_report: Option<FrameReport>,
    pub pointer: PointerState,
    pub touches: Vec<TouchEvent>,
    pub keyboard: Vec<KeyboardEvent>,
    pub text_input: Vec<TextInputEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppView {
    Compose(Node),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    Click {
        action_id: ActionId,
    },
    ToggleChanged {
        action_id: ActionId,
        checked: bool,
    },
    FocusChanged {
        action_id: ActionId,
        focused: bool,
    },
    KeyInput {
        action_id: ActionId,
        key: Key,
        state: KeyState,
        repeat: bool,
        modifiers: KeyboardModifiers,
    },
    TextInput {
        action_id: ActionId,
        text: String,
    },
}

pub trait App {
    type Message: 'static;

    fn update(&mut self, _frame: &AppFrame, _message: Self::Message) {}

    fn on_ui_event(&mut self, _frame: &AppFrame, _event: &UiEvent) {}

    fn render(&mut self, frame: &AppFrame) -> AppView;

    fn animation_interval(&self, _frame: &AppFrame) -> Option<Duration> {
        None
    }
}
