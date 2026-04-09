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
        Self { dx, dy, blur, color }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Modifier {
    Padding(EdgeInsets),
    Background(Color),
    Foreground(Color),
    CornerRadius(f32),
    Spacing(f32),
    Width(f32),
    Height(f32),
    ClipBounds,
    ClipRounded(f32),
    Translate { x: f32, y: f32 },
    Scale { x: f32, y: f32 },
    RotateDegrees(f32),
    TransformOrigin(TransformOrigin),
    Opacity(f32),
    Layer,
    BlendMode(BlendMode),
    Blur(f32),
    DropShadow(DropShadow),
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
}
