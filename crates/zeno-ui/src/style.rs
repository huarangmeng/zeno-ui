use zeno_core::{Color, Transform2D};

use crate::modifier::{
    Alignment, Arrangement, BlendMode, ClipMode, CrossAxisAlignment, DropShadow, Modifier,
    Modifiers, TransformOrigin,
};

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
    pub font_size: Option<f32>,
    pub corner_radius: f32,
    pub spacing: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub clip: Option<ClipMode>,
    pub transform: Transform2D,
    pub transform_origin: TransformOrigin,
    pub content_alignment: Alignment,
    pub arrangement: Arrangement,
    pub cross_axis_alignment: CrossAxisAlignment,
    pub opacity: f32,
    pub layer: bool,
    pub blend_mode: BlendMode,
    pub blur: Option<f32>,
    pub drop_shadow: Option<DropShadow>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            padding: EdgeInsets::default(),
            background: None,
            foreground: Color::BLACK,
            font_size: None,
            corner_radius: 0.0,
            spacing: 0.0,
            width: None,
            height: None,
            clip: None,
            transform: Transform2D::identity(),
            transform_origin: TransformOrigin::new(0.0, 0.0),
            content_alignment: Alignment::TOP_START,
            arrangement: Arrangement::Start,
            cross_axis_alignment: CrossAxisAlignment::Start,
            opacity: 1.0,
            layer: false,
            blend_mode: BlendMode::Normal,
            blur: None,
            drop_shadow: None,
        }
    }
}

impl Style {
    pub fn apply_modifier(&mut self, modifier: &Modifier) {
        match modifier {
            Modifier::Padding(padding) => self.padding = *padding,
            Modifier::Background(color) => self.background = Some(*color),
            Modifier::Foreground(color) => self.foreground = *color,
            Modifier::FontSize(font_size) => self.font_size = Some((*font_size).max(0.0)),
            Modifier::CornerRadius(radius) => self.corner_radius = *radius,
            Modifier::Spacing(spacing) => self.spacing = *spacing,
            Modifier::FixedSize { width, height } => {
                self.width = Some((*width).max(0.0));
                self.height = Some((*height).max(0.0));
            }
            Modifier::Width(width) => self.width = Some(*width),
            Modifier::Height(height) => self.height = Some(*height),
            Modifier::ClipBounds => self.clip = Some(ClipMode::Bounds),
            Modifier::ClipRounded(radius) => {
                self.clip = Some(ClipMode::RoundedBounds { radius: *radius });
            }
            Modifier::Translate { x, y } => {
                self.transform = self.transform.then(Transform2D::translation(*x, *y));
            }
            Modifier::Scale { x, y } => {
                self.transform = self.transform.then(Transform2D::scale(*x, *y));
            }
            Modifier::RotateDegrees(deg) => {
                self.transform = self.transform.then(Transform2D::rotation_degrees(*deg));
            }
            Modifier::TransformOrigin(origin) => {
                self.transform_origin = *origin;
            }
            Modifier::ContentAlignment(alignment) => {
                self.content_alignment = *alignment;
            }
            Modifier::Arrangement(arrangement) => {
                self.arrangement = *arrangement;
            }
            Modifier::CrossAxisAlignment(alignment) => {
                self.cross_axis_alignment = *alignment;
            }
            Modifier::Opacity(opacity) => {
                self.opacity = opacity.clamp(0.0, 1.0);
            }
            Modifier::Layer => {
                self.layer = true;
            }
            Modifier::BlendMode(mode) => {
                self.blend_mode = *mode;
            }
            Modifier::Blur(sigma) => {
                self.blur = Some((*sigma).max(0.0));
            }
            Modifier::DropShadow(shadow) => {
                self.drop_shadow = Some(*shadow);
            }
        }
    }

    #[must_use]
    pub fn from_modifiers(modifiers: &Modifiers) -> Self {
        let mut style = Self::default();
        for modifier in modifiers.iter() {
            style.apply_modifier(modifier);
        }
        style
    }
}
