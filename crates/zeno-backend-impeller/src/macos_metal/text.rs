use font_kit::source::SystemSource;
use fontdue::Font;
use zeno_text::TextLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphCacheKey {
    pub glyph_id: u16,
    pub font_size_bits: u32,
}

#[derive(Debug, Clone)]
pub struct CachedGlyph {
    pub metrics: fontdue::Metrics,
    pub bitmap: Vec<u8>,
}

pub fn load_system_font() -> Option<Font> {
    let families = [
        "PingFang SC",
        "Helvetica Neue",
        "Arial",
        "Noto Sans CJK SC",
        "Noto Sans",
    ];
    for family in families {
        if let Ok(handle) = SystemSource::new().select_family_by_name(family)
            && let Some(font_handle) = handle.fonts().first()
            && let Ok(font) = font_handle.load()
            && let Some(bytes) = font.copy_font_data()
            && let Ok(parsed) = Font::from_bytes(bytes.as_slice(), fontdue::FontSettings::default())
        {
            return Some(parsed);
        }
    }
    None
}

pub fn glyph_cache_key(glyph_id: u16, font_size: f32) -> GlyphCacheKey {
    GlyphCacheKey {
        glyph_id,
        font_size_bits: font_size.max(12.0).to_bits(),
    }
}

pub fn rasterize_glyph(font: &Font, glyph_id: u16, glyph: char, font_size: f32) -> CachedGlyph {
    let (metrics, bitmap) = if glyph_id == 0 {
        font.rasterize(glyph, font_size.max(12.0))
    } else {
        font.rasterize_indexed(glyph_id, font_size.max(12.0))
    };
    CachedGlyph { metrics, bitmap }
}

pub fn rasterize_layout(
    layout: &TextLayout,
    cached_glyph: impl FnMut(u16, char, f32) -> Option<CachedGlyph>,
) -> Option<(Vec<u8>, u32, u32)> {
    let width = layout.metrics.width.ceil().max(1.0) as usize;
    let height = layout.metrics.height.ceil().max(1.0) as usize;
    let mut alpha = vec![0u8; width * height];
    let mut cached_glyph = cached_glyph;
    for glyph in &layout.glyphs {
        let rasterized = cached_glyph(glyph.glyph_id, glyph.glyph, layout.paragraph.font_size)?;
        let glyph_x = (glyph.x + rasterized.metrics.xmin as f32).max(0.0) as usize;
        let baseline = layout.metrics.ascent + glyph.baseline_y;
        let glyph_y = (baseline
            - rasterized.metrics.height as f32
            - rasterized.metrics.ymin as f32)
            .max(0.0) as usize;
        for row in 0..rasterized.metrics.height {
            for col in 0..rasterized.metrics.width {
                let src = rasterized.bitmap[row * rasterized.metrics.width + col];
                if src == 0 {
                    continue;
                }
                let x = glyph_x + col;
                let y = glyph_y + row;
                if x < width && y < height {
                    alpha[y * width + x] = alpha[y * width + x].max(src);
                }
            }
        }
    }
    Some((alpha, width as u32, height as u32))
}
