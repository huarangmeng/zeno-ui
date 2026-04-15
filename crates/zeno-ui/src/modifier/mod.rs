use zeno_core::Color;

use crate::{EdgeInsets, Style};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClipMode {
    Bounds,
    RoundedBounds { radius: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformOrigin {
    pub x: f32,
    pub y: f32,
}

impl TransformOrigin {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAlignment {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossAxisAlignment {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Arrangement {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alignment {
    pub horizontal: HorizontalAlignment,
    pub vertical: VerticalAlignment,
}

impl Alignment {
    pub const TOP_START: Self = Self::new(HorizontalAlignment::Start, VerticalAlignment::Top);
    pub const TOP_CENTER: Self = Self::new(HorizontalAlignment::Center, VerticalAlignment::Top);
    pub const TOP_END: Self = Self::new(HorizontalAlignment::End, VerticalAlignment::Top);
    pub const CENTER_START: Self = Self::new(HorizontalAlignment::Start, VerticalAlignment::Center);
    pub const CENTER: Self = Self::new(HorizontalAlignment::Center, VerticalAlignment::Center);
    pub const CENTER_END: Self = Self::new(HorizontalAlignment::End, VerticalAlignment::Center);
    pub const BOTTOM_START: Self = Self::new(HorizontalAlignment::Start, VerticalAlignment::Bottom);
    pub const BOTTOM_CENTER: Self =
        Self::new(HorizontalAlignment::Center, VerticalAlignment::Bottom);
    pub const BOTTOM_END: Self = Self::new(HorizontalAlignment::End, VerticalAlignment::Bottom);

    #[must_use]
    pub const fn new(horizontal: HorizontalAlignment, vertical: VerticalAlignment) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }
}

impl Default for Alignment {
    fn default() -> Self {
        Self::TOP_START
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropShadow {
    pub dx: f32,
    pub dy: f32,
    pub blur: f32,
    pub color: Color,
}

impl DropShadow {
    #[must_use]
    pub const fn new(dx: f32, dy: f32, blur: f32, color: Color) -> Self {
        Self {
            dx,
            dy,
            blur,
            color,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionRole {
    Button,
    ToggleButton,
    Checkbox,
    Switch,
    Scroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InteractionState {
    pub role: Option<InteractionRole>,
    pub action: Option<ActionId>,
    pub checked: Option<bool>,
    pub enabled: bool,
    pub focusable: bool,
    pub accepts_text_input: bool,
}

impl InteractionState {
    #[must_use]
    pub const fn is_interactive(self) -> bool {
        self.enabled
            && (self.role.is_some()
                || self.action.is_some()
                || self.focusable
                || self.accepts_text_input
                || self.checked.is_some())
    }

    #[must_use]
    pub const fn is_focusable(self) -> bool {
        self.enabled && (self.focusable || self.action.is_some() || self.accepts_text_input)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Modifier {
    Padding(EdgeInsets),
    Background(Color),
    Foreground(Color),
    FontSize(f32),
    CornerRadius(f32),
    Spacing(f32),
    FixedSize { width: f32, height: f32 },
    Width(f32),
    Height(f32),
    ClipBounds,
    ClipRounded(f32),
    Translate { x: f32, y: f32 },
    Scale { x: f32, y: f32 },
    RotateDegrees(f32),
    TransformOrigin(TransformOrigin),
    ContentAlignment(Alignment),
    Arrangement(Arrangement),
    CrossAxisAlignment(CrossAxisAlignment),
    Opacity(f32),
    Layer,
    BlendMode(BlendMode),
    Blur(f32),
    DropShadow(DropShadow),
    InteractionRole(InteractionRole),
    Action(ActionId),
    Checked(bool),
    Enabled(bool),
    Focusable,
    AcceptTextInput,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Modifiers(Vec<Modifier>);

impl Modifiers {
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, modifier: Modifier) {
        self.0.push(modifier);
    }

    pub fn extend(&mut self, modifiers: impl IntoIterator<Item = Modifier>) {
        self.0.extend(modifiers);
    }

    #[must_use]
    pub fn iter(&self) -> impl Iterator<Item = &Modifier> {
        self.0.iter()
    }

    #[must_use]
    pub fn resolve_style(&self) -> Style {
        Style::from_modifiers(self)
    }

    #[must_use]
    pub fn resolve_interaction(&self) -> InteractionState {
        let mut state = InteractionState::default();
        state.enabled = true;
        for modifier in self.iter() {
            match modifier {
                Modifier::InteractionRole(role) => state.role = Some(*role),
                Modifier::Action(action) => state.action = Some(*action),
                Modifier::Checked(checked) => state.checked = Some(*checked),
                Modifier::Enabled(enabled) => state.enabled = *enabled,
                Modifier::Focusable => state.focusable = true,
                Modifier::AcceptTextInput => {
                    state.accepts_text_input = true;
                    state.focusable = true;
                }
                _ => {}
            }
        }
        state
    }
}
