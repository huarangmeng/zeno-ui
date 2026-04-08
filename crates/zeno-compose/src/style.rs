use zeno_core::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl EdgeInsets {
    #[must_use]
    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    #[must_use]
    pub const fn horizontal_vertical(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }

    #[must_use]
    pub const fn horizontal(self) -> f32 {
        self.left + self.right
    }

    #[must_use]
    pub const fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

impl Default for EdgeInsets {
    fn default() -> Self {
        Self::all(0.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub padding: EdgeInsets,
    pub background: Option<Color>,
    pub foreground: Color,
    pub corner_radius: f32,
    pub spacing: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            padding: EdgeInsets::default(),
            background: None,
            foreground: Color::BLACK,
            corner_radius: 0.0,
            spacing: 0.0,
            width: None,
            height: None,
        }
    }
}
