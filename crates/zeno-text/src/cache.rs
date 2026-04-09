use std::collections::HashMap;
use std::sync::Mutex;

use fontdue::Font;

use crate::{TextLayout, TextParagraphKey};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextCacheStats {
    pub entries: usize,
    pub hits: usize,
    pub misses: usize,
}

pub trait TextCache: Send + Sync {
    fn get(&self, key: TextParagraphKey) -> Option<TextLayout>;

    fn insert(&self, key: TextParagraphKey, layout: TextLayout);

    fn stats(&self) -> TextCacheStats;

    fn reset(&self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphRasterKey {
    pub glyph_id: u16,
    pub glyph: char,
    pub font_size_bits: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphRasterMetrics {
    pub width: usize,
    pub height: usize,
    pub xmin: i32,
    pub ymin: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedGlyph {
    pub metrics: GlyphRasterMetrics,
    pub bitmap: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct GlyphRasterCache {
    inner: Mutex<GlyphRasterCacheState>,
}

#[derive(Debug, Default)]
struct GlyphRasterCacheState {
    glyphs: HashMap<GlyphRasterKey, CachedGlyph>,
    hits: usize,
    misses: usize,
}

#[derive(Debug, Default)]
pub struct ParagraphTextCache {
    inner: Mutex<ParagraphTextCacheState>,
}

#[derive(Debug, Default)]
struct ParagraphTextCacheState {
    layouts: HashMap<TextParagraphKey, TextLayout>,
    hits: usize,
    misses: usize,
}

impl TextCache for ParagraphTextCache {
    fn get(&self, key: TextParagraphKey) -> Option<TextLayout> {
        let mut inner = self.inner.lock().expect("paragraph text cache");
        let layout = inner.layouts.get(&key).cloned();
        if layout.is_some() {
            inner.hits += 1;
        } else {
            inner.misses += 1;
        }
        layout
    }

    fn insert(&self, key: TextParagraphKey, layout: TextLayout) {
        let mut inner = self.inner.lock().expect("paragraph text cache");
        inner.layouts.insert(key, layout);
    }

    fn stats(&self) -> TextCacheStats {
        let inner = self.inner.lock().expect("paragraph text cache");
        TextCacheStats {
            entries: inner.layouts.len(),
            hits: inner.hits,
            misses: inner.misses,
        }
    }

    fn reset(&self) {
        let mut inner = self.inner.lock().expect("paragraph text cache");
        inner.layouts.clear();
        inner.hits = 0;
        inner.misses = 0;
    }
}

impl GlyphRasterCache {
    #[must_use]
    pub fn glyph_key(glyph_id: u16, glyph: char, font_size: f32) -> GlyphRasterKey {
        GlyphRasterKey {
            glyph_id,
            glyph,
            font_size_bits: font_size.max(12.0).to_bits(),
        }
    }

    pub fn get_or_rasterize(
        &self,
        font: &Font,
        glyph_id: u16,
        glyph: char,
        font_size: f32,
    ) -> CachedGlyph {
        let key = Self::glyph_key(glyph_id, glyph, font_size);
        let mut inner = self.inner.lock().expect("glyph raster cache");
        if let Some(cached) = inner.glyphs.get(&key).cloned() {
            inner.hits += 1;
            return cached;
        }
        inner.misses += 1;
        let (metrics, bitmap) = if glyph_id == 0 {
            font.rasterize(glyph, font_size.max(12.0))
        } else {
            font.rasterize_indexed(glyph_id, font_size.max(12.0))
        };
        let cached = CachedGlyph {
            metrics: GlyphRasterMetrics {
                width: metrics.width,
                height: metrics.height,
                xmin: metrics.xmin,
                ymin: metrics.ymin,
            },
            bitmap,
        };
        inner.glyphs.insert(key, cached.clone());
        cached
    }

    #[must_use]
    pub fn stats(&self) -> TextCacheStats {
        let inner = self.inner.lock().expect("glyph raster cache");
        TextCacheStats {
            entries: inner.glyphs.len(),
            hits: inner.hits,
            misses: inner.misses,
        }
    }

    pub fn reset(&self) {
        let mut inner = self.inner.lock().expect("glyph raster cache");
        inner.glyphs.clear();
        inner.hits = 0;
        inner.misses = 0;
    }
}
