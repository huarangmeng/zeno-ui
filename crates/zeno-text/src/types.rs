use zeno_core::Size;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontDescriptor {
    pub family: String,
    pub weight: u16,
    pub italic: bool,
}

impl Default for FontDescriptor {
    fn default() -> Self {
        Self {
            family: "System".to_string(),
            weight: 400,
            italic: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextParagraph {
    pub text: String,
    pub font: FontDescriptor,
    pub font_size: f32,
    pub max_width: f32,
}

impl TextParagraph {
    #[must_use]
    pub fn new(text: impl Into<String>, max_width: f32) -> Self {
        Self {
            text: text.into(),
            font: FontDescriptor::default(),
            font_size: 16.0,
            max_width,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    pub line_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    pub paragraph: TextParagraph,
    pub metrics: TextMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextCapabilities {
    pub shaping: bool,
    pub line_breaking: bool,
    pub glyph_cache: bool,
}

impl TextCapabilities {
    #[must_use]
    pub const fn minimal() -> Self {
        Self {
            shaping: true,
            line_breaking: true,
            glyph_cache: false,
        }
    }
}

#[must_use]
pub fn line_box(layout: &TextLayout) -> Size {
    Size::new(layout.metrics.width, layout.metrics.height)
}
