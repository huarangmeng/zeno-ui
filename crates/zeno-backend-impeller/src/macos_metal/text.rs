use font_kit::source::SystemSource;
use fontdue::Font;

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

pub fn rasterize_text(font: &Font, text: &str, font_size: f32) -> Option<(Vec<u8>, u32, u32, f32)> {
    let size = font_size.max(12.0);
    let line_metrics = font.horizontal_line_metrics(size)?;
    let mut glyphs = Vec::new();
    let mut total_width = 0.0f32;
    let mut max_height = 0usize;

    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);
        total_width += metrics.advance_width.max(1.0);
        max_height = max_height.max(metrics.height.max(line_metrics.new_line_size.ceil() as usize));
        glyphs.push((metrics, bitmap));
    }

    let width = total_width.ceil().max(1.0) as usize;
    let height = max_height.max(1);
    let mut alpha = vec![0u8; width * height];
    let baseline = line_metrics.ascent.ceil() as isize;
    let mut pen_x = 0.0f32;

    for (metrics, bitmap) in glyphs {
        let glyph_x = (pen_x + metrics.xmin as f32).max(0.0) as usize;
        let glyph_y = (baseline - metrics.height as isize - metrics.ymin as isize).max(0) as usize;
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let src = bitmap[row * metrics.width + col];
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
        pen_x += metrics.advance_width;
    }

    Some((alpha, width as u32, height as u32, baseline as f32))
}
