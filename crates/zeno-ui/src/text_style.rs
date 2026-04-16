use zeno_core::Color;
use zeno_text::{FontDescriptor, FontFeature, FontFeatures, FontWeight};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub color: Color,
    pub font_size: Option<f32>,
    pub font: FontDescriptor,
    pub letter_spacing: Option<f32>,
    pub line_height: Option<f32>,
    pub text_align: Option<TextAlign>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
            font_size: None,
            font: FontDescriptor::default(),
            letter_spacing: None,
            line_height: None,
            text_align: None,
        }
    }
}

impl TextStyle {
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    #[must_use]
    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size.max(0.0));
        self
    }

    #[must_use]
    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.font.family = family.into();
        self
    }

    #[must_use]
    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.font.weight = weight;
        self
    }

    #[must_use]
    pub fn italic(mut self) -> Self {
        self.font.italic = true;
        self
    }

    #[must_use]
    pub fn font_feature(mut self, feature: FontFeature) -> Self {
        self.font.features.insert(feature);
        self
    }

    #[must_use]
    pub fn font_features(mut self, features: FontFeatures) -> Self {
        self.font.features = features;
        self
    }

    #[must_use]
    pub fn letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = Some(spacing);
        self
    }

    #[must_use]
    pub fn line_height(mut self, height: f32) -> Self {
        self.line_height = Some(height.max(0.0));
        self
    }

    #[must_use]
    pub fn text_align(mut self, align: TextAlign) -> Self {
        self.text_align = Some(align);
        self
    }

    pub fn merge(&mut self, other: &Self) {
        self.color = other.color;
        if other.font_size.is_some() {
            self.font_size = other.font_size;
        }
        if other.font.family != FontDescriptor::default().family {
            self.font.family = other.font.family.clone();
        }
        if other.font.weight != FontWeight::NORMAL {
            self.font.weight = other.font.weight;
        }
        if other.font.italic {
            self.font.italic = true;
        }
        if other.font.features != FontFeatures::empty() {
            self.font.features = other.font.features;
        }
        if other.letter_spacing.is_some() {
            self.letter_spacing = other.letter_spacing;
        }
        if other.line_height.is_some() {
            self.line_height = other.line_height;
        }
        if other.text_align.is_some() {
            self.text_align = other.text_align;
        }
    }
}
