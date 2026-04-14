use zeno_core::Point;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TouchEvent {
    pub id: u64,
    pub phase: TouchPhase,
    pub position: Point,
    pub force: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyboardModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Character(String),
    Enter,
    Space,
    Tab,
    Escape,
    Backspace,
    Delete,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardEvent {
    pub key: Key,
    pub state: KeyState,
    pub repeat: bool,
    pub modifiers: KeyboardModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextInputEvent {
    pub text: String,
}
