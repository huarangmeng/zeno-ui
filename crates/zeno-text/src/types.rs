use zeno_core::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontFeature {
    TabularNumbers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const THIN: Self = Self(100);
    pub const EXTRA_LIGHT: Self = Self(200);
    pub const LIGHT: Self = Self(300);
    pub const NORMAL: Self = Self(400);
    pub const MEDIUM: Self = Self(500);
    pub const SEMI_BOLD: Self = Self(600);
    pub const BOLD: Self = Self(700);
    pub const EXTRA_BOLD: Self = Self(800);
    pub const BLACK: Self = Self(900);

    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FontFeatures(u16);

impl FontFeatures {
    const TABULAR_NUMBERS: u16 = 1 << 0;

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn tabular_numbers() -> Self {
        Self(Self::TABULAR_NUMBERS)
    }

    pub fn insert(&mut self, feature: FontFeature) {
        match feature {
            FontFeature::TabularNumbers => self.0 |= Self::TABULAR_NUMBERS,
        }
    }

    #[must_use]
    pub const fn contains(self, feature: FontFeature) -> bool {
        match feature {
            FontFeature::TabularNumbers => (self.0 & Self::TABULAR_NUMBERS) != 0,
        }
    }

    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FontDescriptor {
    pub family: String,
    pub weight: FontWeight,
    pub italic: bool,
    pub features: FontFeatures,
}

impl Default for FontDescriptor {
    fn default() -> Self {
        Self {
            family: "System".to_string(),
            weight: FontWeight::NORMAL,
            italic: false,
            features: FontFeatures::empty(),
        }
    }
}

impl FontDescriptor {
    #[must_use]
    pub fn cache_hash(&self) -> u64 {
        stable_hash(self.family.as_bytes())
            ^ ((self.weight.0 as u64) << 32)
            ^ ((self.italic as u64) << 48)
            ^ ((self.features.bits() as u64) << 56)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextParagraph {
    pub text: String,
    pub font: FontDescriptor,
    pub font_size: f32,
    pub max_width: f32,
    pub letter_spacing: Option<f32>,
    pub line_height: Option<f32>,
    pub text_align: Option<TextAlign>,
    pub max_lines: Option<usize>,
    pub soft_wrap: bool,
    pub overflow: TextOverflow,
    pub alignment_width: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextParagraphKey {
    pub text_hash: u64,
    pub family_hash: u64,
    pub weight: FontWeight,
    pub italic: bool,
    pub features_bits: u16,
    pub font_size_bits: u32,
    pub max_width_bits: u32,
    pub letter_spacing_bits: Option<u32>,
    pub line_height_bits: Option<u32>,
    pub text_align_bits: Option<u8>,
    pub max_lines: Option<usize>,
    pub soft_wrap: bool,
    pub overflow_bits: u8,
    pub alignment_width_bits: Option<u32>,
}

impl TextParagraph {
    #[must_use]
    pub fn new(text: impl Into<String>, max_width: f32) -> Self {
        Self {
            text: text.into(),
            font: FontDescriptor::default(),
            font_size: 16.0,
            max_width,
            letter_spacing: None,
            line_height: None,
            text_align: None,
            max_lines: None,
            soft_wrap: true,
            overflow: TextOverflow::Clip,
            alignment_width: None,
        }
    }

    #[must_use]
    pub fn cache_key(&self) -> TextParagraphKey {
        TextParagraphKey {
            text_hash: stable_hash(self.text.as_bytes()),
            family_hash: stable_hash(self.font.family.as_bytes()),
            weight: self.font.weight,
            italic: self.font.italic,
            features_bits: self.font.features.bits(),
            font_size_bits: self.font_size.to_bits(),
            max_width_bits: self.max_width.to_bits(),
            letter_spacing_bits: self.letter_spacing.map(f32::to_bits),
            line_height_bits: self.line_height.map(f32::to_bits),
            text_align_bits: self.text_align.map(|a| a as u8),
            max_lines: self.max_lines,
            soft_wrap: self.soft_wrap,
            overflow_bits: self.overflow as u8,
            alignment_width_bits: self.alignment_width.map(f32::to_bits),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    pub line_count: usize,
    pub line_height: f32,
    pub ascent: f32,
    pub descent: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedGlyph {
    pub glyph: char,
    pub glyph_id: u16,
    pub x: f32,
    pub baseline_y: f32,
    pub advance: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    pub paragraph: TextParagraph,
    pub metrics: TextMetrics,
    pub glyphs: Vec<ShapedGlyph>,
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
        let ls_hash = match self.letter_spacing_bits {
            Some(b) => (b as u64).rotate_left(13),
            None => 0u64,
        };
        let lh_hash = match self.line_height_bits {
            Some(b) => (b as u64).rotate_left(19),
            None => 0u64,
        };
        let ta_hash = match self.text_align_bits {
            Some(b) => (b as u64) << 24,
            None => 0u64,
        };
        let ml_hash = match self.max_lines {
            Some(lines) => (lines as u64).rotate_left(29),
            None => 0u64,
        };
        let aw_hash = match self.alignment_width_bits {
            Some(bits) => (bits as u64).rotate_left(11),
            None => 0u64,
        };
        self.text_hash
            ^ self.family_hash.rotate_left(7)
            ^ ((self.weight.0 as u64) << 32)
            ^ ((self.italic as u64) << 48)
            ^ ((self.features_bits as u64) << 56)
            ^ ((self.font_size_bits as u64) << 8)
            ^ (self.max_width_bits as u64)
            ^ ls_hash
            ^ lh_hash
            ^ ta_hash
            ^ ml_hash
            ^ ((self.soft_wrap as u64) << 20)
            ^ ((self.overflow_bits as u64) << 28)
            ^ aw_hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextCapabilities {
    pub shaping: bool,
    pub line_breaking: bool,
    pub paragraph_cache: bool,
    pub glyph_cache: bool,
}

impl TextCapabilities {
    #[must_use]
    pub const fn minimal() -> Self {
        Self {
            shaping: true,
            line_breaking: true,
            paragraph_cache: false,
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
