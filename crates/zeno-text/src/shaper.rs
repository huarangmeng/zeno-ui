use std::sync::OnceLock;

use font_kit::source::SystemSource;
use rustybuzz::UnicodeBuffer;

use crate::{ShapedGlyph, TextLayout, TextMetrics, TextParagraph};

pub trait TextShaper: Send + Sync {
    fn name(&self) -> &'static str;

    fn shape(&self, paragraph: TextParagraph) -> TextLayout;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FallbackTextShaper;

impl TextShaper for FallbackTextShaper {
    fn name(&self) -> &'static str {
        "fallback-shaper"
    }

    fn shape(&self, paragraph: TextParagraph) -> TextLayout {
        if let Some(layout) = shape_with_system_font(&paragraph) {
            return layout;
        }
        fallback_shape(paragraph)
    }
}

fn fallback_shape(paragraph: TextParagraph) -> TextLayout {
    let line_height = paragraph.font_size * 1.4;
    let ascent = paragraph.font_size * 0.8;
    let descent = paragraph.font_size * 0.2;
    let wrap_width = if paragraph.max_width <= 0.0 {
        f32::MAX
    } else {
        paragraph.max_width
    };
    let mut glyphs = Vec::new();
    let mut pen_x = 0.0f32;
    let mut baseline_y = 0.0f32;
    let mut max_width = 0.0f32;
    let mut line_count = 1usize;

    for glyph in paragraph.text.chars() {
        if glyph == '\n' {
            max_width = max_width.max(pen_x);
            pen_x = 0.0;
            baseline_y += line_height;
            line_count += 1;
            continue;
        }
        let advance = estimated_advance(glyph, paragraph.font_size);
        if pen_x > 0.0 && pen_x + advance > wrap_width {
            max_width = max_width.max(pen_x);
            pen_x = 0.0;
            baseline_y += line_height;
            line_count += 1;
        }
        glyphs.push(ShapedGlyph {
            glyph,
            glyph_id: 0,
            x: pen_x,
            baseline_y,
            advance,
        });
        pen_x += advance;
        max_width = max_width.max(pen_x);
    }
    let metrics = TextMetrics {
        width: max_width.min(wrap_width),
        height: line_height * line_count as f32,
        line_count,
        line_height,
        ascent,
        descent,
    };
    TextLayout {
        paragraph,
        metrics,
        glyphs,
    }
}

fn shape_with_system_font(paragraph: &TextParagraph) -> Option<TextLayout> {
    let font_data = system_font_data()?;
    let face = rustybuzz::Face::from_slice(font_data, 0)?;
    let units_per_em = face.units_per_em() as f32;
    let scale = paragraph.font_size.max(1.0) / units_per_em.max(1.0);
    let line_height = paragraph.font_size * 1.4;
    let ascent = face.ascender() as f32 * scale;
    let descent = (-(face.descender() as f32)).max(0.0) * scale;
    let wrap_width = if paragraph.max_width <= 0.0 {
        f32::MAX
    } else {
        paragraph.max_width
    };
    let mut glyphs = Vec::new();
    let mut max_width = 0.0f32;
    let mut line_count = 0usize;
    for line in paragraph.text.split('\n') {
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(line);
        let shaped = rustybuzz::shape(&face, &[], buffer);
        let infos = shaped.glyph_infos();
        let positions = shaped.glyph_positions();
        let mut pen_x = 0.0f32;
        let mut baseline_y = line_count as f32 * line_height;
        for (info, position) in infos.iter().zip(positions.iter()) {
            let advance = position.x_advance as f32 * scale;
            let x_offset = position.x_offset as f32 * scale;
            let y_offset = -(position.y_offset as f32 * scale);
            if pen_x > 0.0 && pen_x + advance > wrap_width {
                max_width = max_width.max(pen_x);
                pen_x = 0.0;
                baseline_y += line_height;
                line_count += 1;
            }
            let cluster_index = usize::try_from(info.cluster).unwrap_or(0);
            let glyph_char = cluster_char(line, cluster_index).unwrap_or(' ');
            glyphs.push(ShapedGlyph {
                glyph: glyph_char,
                glyph_id: info.glyph_id as u16,
                x: pen_x + x_offset,
                baseline_y: baseline_y + y_offset,
                advance,
            });
            pen_x += advance;
            max_width = max_width.max(pen_x);
        }
        line_count += 1;
    }
    if paragraph.text.is_empty() {
        let line_height = paragraph.font_size * 1.4;
        return Some(TextLayout {
            paragraph: paragraph.clone(),
            metrics: TextMetrics {
                width: 0.0,
                height: line_height,
                line_count: 1,
                line_height,
                ascent,
                descent,
            },
            glyphs,
        });
    }
    Some(TextLayout {
        paragraph: paragraph.clone(),
        metrics: TextMetrics {
            width: max_width.min(wrap_width),
            height: line_height * line_count.max(1) as f32,
            line_count: line_count.max(1),
            line_height,
            ascent,
            descent,
        },
        glyphs,
    })
}

fn estimated_advance(glyph: char, font_size: f32) -> f32 {
    let factor = if glyph.is_whitespace() {
        0.35
    } else if glyph.is_ascii_punctuation() {
        0.45
    } else if glyph.is_ascii() {
        0.55
    } else {
        1.0
    };
    (font_size * factor).max(1.0)
}

fn system_font_data() -> Option<&'static [u8]> {
    static FONT_DATA: OnceLock<Option<&'static [u8]>> = OnceLock::new();
    FONT_DATA.get_or_init(load_system_font_data).to_owned()
}

fn load_system_font_data() -> Option<&'static [u8]> {
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
        {
            let leaked: &'static mut [u8] = Box::leak(bytes.as_slice().to_vec().into_boxed_slice());
            return Some(leaked);
        }
    }
    None
}

fn cluster_char(text: &str, cluster_index: usize) -> Option<char> {
    text.get(cluster_index..)?.chars().next()
}

#[cfg(test)]
mod tests {
    use super::{FallbackTextShaper, TextShaper};
    use crate::TextParagraph;

    #[test]
    fn fallback_shaper_emits_wrapped_glyph_positions() {
        let layout = FallbackTextShaper.shape(TextParagraph::new("wrap me", 30.0));

        assert!(layout.metrics.line_count >= 2);
        assert!(!layout.glyphs.is_empty());
        assert!(layout.glyphs.windows(2).any(|pair| pair[1].baseline_y > pair[0].baseline_y));
    }
}
