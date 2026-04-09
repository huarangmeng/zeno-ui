use zeno_text::{CachedGlyph, TextLayout};

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
        let glyph_y = (baseline - rasterized.metrics.height as f32 - rasterized.metrics.ymin as f32)
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
