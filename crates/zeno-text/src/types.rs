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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextParagraphKey {
    pub text_hash: u64,
    pub family_hash: u64,
    pub weight: u16,
    pub italic: bool,
    pub font_size_bits: u32,
    pub max_width_bits: u32,
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

    #[must_use]
    pub fn cache_key(&self) -> TextParagraphKey {
        TextParagraphKey {
            text_hash: stable_hash(self.text.as_bytes()),
            family_hash: stable_hash(self.font.family.as_bytes()),
            weight: self.font.weight,
            italic: self.font.italic,
            font_size_bits: self.font_size.to_bits(),
            max_width_bits: self.max_width.to_bits(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    pub line_count: usize,
    pub ascent: f32,
    pub descent: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    pub paragraph: TextParagraph,
    pub metrics: TextMetrics,
}

impl TextLayout {
    #[must_use]
    pub fn cache_key(&self) -> TextParagraphKey {
        self.paragraph.cache_key()
    }
}

impl TextParagraphKey {
    #[must_use]
    pub const fn stable_hash(self) -> u64 {
        self.text_hash
            ^ self.family_hash.rotate_left(7)
            ^ ((self.weight as u64) << 32)
            ^ ((self.italic as u64) << 48)
            ^ ((self.font_size_bits as u64) << 8)
            ^ (self.max_width_bits as u64)
    }
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

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
