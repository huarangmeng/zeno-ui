use std::str::FromStr;

use rustybuzz::{Feature, UnicodeBuffer};

use crate::{
    FontFeature, ShapedGlyph, TextAlign, TextLayout, TextMetrics, TextOverflow, TextParagraph,
    font::system_font_face_for,
};

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
        fallback_shape(paragraph)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemTextShaper;

impl TextShaper for SystemTextShaper {
    fn name(&self) -> &'static str {
        "system-shaper"
    }

    fn shape(&self, paragraph: TextParagraph) -> TextLayout {
        shape_with_system_font(&paragraph).unwrap_or_else(|| fallback_shape(paragraph))
    }
}

#[derive(Debug, Clone)]
struct PositionedGlyph {
    glyph: char,
    glyph_id: u16,
    x_offset: f32,
    y_offset: f32,
    advance: f32,
}

#[derive(Debug, Clone, Default)]
struct VisualLine {
    glyphs: Vec<PositionedGlyph>,
    width: f32,
}

fn fallback_shape(paragraph: TextParagraph) -> TextLayout {
    let line_height = paragraph.line_height.unwrap_or(paragraph.font_size * 1.4);
    let ascent = paragraph.font_size * 0.8;
    let descent = paragraph.font_size * 0.2;
    let letter_spacing = paragraph.letter_spacing.unwrap_or(0.0);
    let lines = paragraph
        .text
        .split('\n')
        .map(|line| {
            line.chars()
                .map(|glyph| PositionedGlyph {
                    glyph,
                    glyph_id: 0,
                    x_offset: 0.0,
                    y_offset: 0.0,
                    advance: estimated_advance(glyph, paragraph.font_size) + letter_spacing,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let ellipsis = vec![PositionedGlyph {
        glyph: '…',
        glyph_id: 0,
        x_offset: 0.0,
        y_offset: 0.0,
        advance: estimated_advance('…', paragraph.font_size) + letter_spacing,
    }];
    build_layout(paragraph, ascent, descent, line_height, lines, ellipsis)
}

fn shape_with_system_font(paragraph: &TextParagraph) -> Option<TextLayout> {
    let face = system_font_face_for(&paragraph.font)?;
    let units_per_em = face.units_per_em() as f32;
    let scale = paragraph.font_size.max(1.0) / units_per_em.max(1.0);
    let line_height = paragraph.line_height.unwrap_or(paragraph.font_size * 1.4);
    let ascent = face.ascender() as f32 * scale;
    let descent = (-(face.descender() as f32)).max(0.0) * scale;
    let letter_spacing = paragraph.letter_spacing.unwrap_or(0.0);
    let features = rustybuzz_features(paragraph);
    let lines = paragraph
        .text
        .split('\n')
        .map(|line| {
            let mut buffer = UnicodeBuffer::new();
            buffer.push_str(line);
            let shaped = rustybuzz::shape(&face, &features, buffer);
            let infos = shaped.glyph_infos();
            let positions = shaped.glyph_positions();
            infos.iter()
                .zip(positions.iter())
                .map(|(info, position)| {
                    let cluster_index = usize::try_from(info.cluster).unwrap_or(0);
                    let glyph_char = cluster_char(line, cluster_index).unwrap_or(' ');
                    PositionedGlyph {
                        glyph: glyph_char,
                        glyph_id: info.glyph_id as u16,
                        x_offset: position.x_offset as f32 * scale,
                        y_offset: -(position.y_offset as f32 * scale),
                        advance: position.x_advance as f32 * scale + letter_spacing,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let ellipsis = {
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str("…");
        let shaped = rustybuzz::shape(&face, &features, buffer);
        let infos = shaped.glyph_infos();
        let positions = shaped.glyph_positions();
        infos.iter()
            .zip(positions.iter())
            .map(|(info, position)| PositionedGlyph {
                glyph: '…',
                glyph_id: info.glyph_id as u16,
                x_offset: position.x_offset as f32 * scale,
                y_offset: -(position.y_offset as f32 * scale),
                advance: position.x_advance as f32 * scale + letter_spacing,
            })
            .collect::<Vec<_>>()
    };
    Some(build_layout(
        paragraph.clone(),
        ascent,
        descent,
        line_height,
        lines,
        ellipsis,
    ))
}

fn build_layout(
    paragraph: TextParagraph,
    ascent: f32,
    descent: f32,
    line_height: f32,
    source_lines: Vec<Vec<PositionedGlyph>>,
    ellipsis: Vec<PositionedGlyph>,
) -> TextLayout {
    let mut lines = wrap_lines(&paragraph, source_lines);
    apply_overflow(&paragraph, &ellipsis, &mut lines);
    if lines.is_empty() {
        lines.push(VisualLine::default());
    }
    let natural_width = lines.iter().fold(0.0f32, |acc, line| acc.max(line.width));
    let aligned_width = paragraph
        .alignment_width
        .map(|width| width.max(0.0))
        .unwrap_or(natural_width);
    let metrics_width = natural_width.max(aligned_width);
    let line_count = lines.len().max(1);
    let mut glyphs = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        let baseline_y = line_index as f32 * line_height;
        let align_offset = alignment_offset(paragraph.text_align, aligned_width, line.width);
        let mut pen_x = 0.0f32;
        for glyph in &line.glyphs {
            glyphs.push(ShapedGlyph {
                glyph: glyph.glyph,
                glyph_id: glyph.glyph_id,
                x: align_offset + pen_x + glyph.x_offset,
                baseline_y: baseline_y + glyph.y_offset,
                advance: glyph.advance,
            });
            pen_x += glyph.advance;
        }
    }
    TextLayout {
        paragraph,
        metrics: TextMetrics {
            width: metrics_width,
            height: line_height * line_count as f32,
            line_count,
            line_height,
            ascent,
            descent,
        },
        glyphs,
    }
}

fn wrap_lines(paragraph: &TextParagraph, source_lines: Vec<Vec<PositionedGlyph>>) -> Vec<VisualLine> {
    let wrap_width = wrap_width(paragraph);
    let mut lines = Vec::new();
    for source_line in source_lines {
        if source_line.is_empty() {
            lines.push(VisualLine::default());
            continue;
        }
        let mut current = VisualLine::default();
        for glyph in source_line {
            if paragraph.soft_wrap
                && wrap_width.is_finite()
                && current.width > 0.0
                && current.width + glyph.advance > wrap_width
            {
                lines.push(current);
                current = VisualLine::default();
            }
            current.width += glyph.advance;
            current.glyphs.push(glyph);
        }
        lines.push(current);
    }
    lines
}

fn apply_overflow(
    paragraph: &TextParagraph,
    ellipsis: &[PositionedGlyph],
    lines: &mut Vec<VisualLine>,
) {
    let max_lines = paragraph.max_lines.unwrap_or(usize::MAX);
    let truncated_by_line_count = lines.len() > max_lines;
    if truncated_by_line_count {
        lines.truncate(max_lines);
    }
    let last_index = lines.len().saturating_sub(1);
    for (index, line) in lines.iter_mut().enumerate() {
        let force_ellipsis =
            truncated_by_line_count && paragraph.overflow == TextOverflow::Ellipsis && index == last_index;
        constrain_line(paragraph, ellipsis, line, force_ellipsis);
    }
}

fn constrain_line(
    paragraph: &TextParagraph,
    ellipsis: &[PositionedGlyph],
    line: &mut VisualLine,
    force_ellipsis: bool,
) {
    let width_limit = wrap_width(paragraph);
    if !width_limit.is_finite() {
        if force_ellipsis {
            append_ellipsis(line, ellipsis);
        }
        return;
    }
    let exceeds_width = line.width > width_limit;
    if !exceeds_width && !force_ellipsis {
        return;
    }
    match paragraph.overflow {
        TextOverflow::Ellipsis if exceeds_width || force_ellipsis => {
            ellipsize_line(line, width_limit, ellipsis);
        }
        TextOverflow::Clip => clip_line(line, width_limit),
        TextOverflow::Ellipsis => {}
    }
}

fn clip_line(line: &mut VisualLine, width_limit: f32) {
    if width_limit <= 0.0 {
        line.glyphs.clear();
        line.width = 0.0;
        return;
    }
    let mut new_width = 0.0f32;
    let mut clipped = Vec::new();
    for glyph in &line.glyphs {
        if new_width + glyph.advance > width_limit {
            break;
        }
        new_width += glyph.advance;
        clipped.push(glyph.clone());
    }
    line.glyphs = clipped;
    line.width = new_width;
}

fn ellipsize_line(line: &mut VisualLine, width_limit: f32, ellipsis: &[PositionedGlyph]) {
    let ellipsis_width = ellipsis.iter().map(|glyph| glyph.advance).sum::<f32>();
    if ellipsis.is_empty() || width_limit <= 0.0 {
        clip_line(line, width_limit);
        return;
    }
    if ellipsis_width > width_limit {
        clip_line(line, width_limit);
        return;
    }
    let available_width = (width_limit - ellipsis_width).max(0.0);
    let mut kept = Vec::new();
    let mut new_width = 0.0f32;
    for glyph in &line.glyphs {
        if new_width + glyph.advance > available_width {
            break;
        }
        new_width += glyph.advance;
        kept.push(glyph.clone());
    }
    line.glyphs = kept;
    line.width = new_width;
    append_ellipsis(line, ellipsis);
}

fn append_ellipsis(line: &mut VisualLine, ellipsis: &[PositionedGlyph]) {
    if ellipsis.is_empty() {
        return;
    }
    line.width += ellipsis.iter().map(|glyph| glyph.advance).sum::<f32>();
    line.glyphs.extend(ellipsis.iter().cloned());
}

fn alignment_offset(text_align: Option<TextAlign>, aligned_width: f32, line_width: f32) -> f32 {
    let free_space = (aligned_width - line_width).max(0.0);
    match text_align.unwrap_or(TextAlign::Start) {
        TextAlign::Start => 0.0,
        TextAlign::Center => free_space * 0.5,
        TextAlign::End => free_space,
    }
}

fn wrap_width(paragraph: &TextParagraph) -> f32 {
    if paragraph.max_width <= 0.0 {
        f32::MAX
    } else {
        paragraph.max_width
    }
}

fn rustybuzz_features(paragraph: &TextParagraph) -> Vec<Feature> {
    let mut features = Vec::new();
    if paragraph.font.features.contains(FontFeature::TabularNumbers)
        && let Ok(feature) = Feature::from_str("tnum")
    {
        features.push(feature);
    }
    features
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

fn cluster_char(text: &str, cluster_index: usize) -> Option<char> {
    text.get(cluster_index..)?.chars().next()
}

#[cfg(test)]
mod tests {
    use super::{FallbackTextShaper, SystemTextShaper, TextShaper};
    use crate::{FontFeature, TextAlign, TextOverflow, TextParagraph};

    #[test]
    fn fallback_shaper_emits_wrapped_glyph_positions() {
        let layout = FallbackTextShaper.shape(TextParagraph::new("wrap me", 30.0));

        assert!(layout.metrics.line_count >= 2);
        assert!(!layout.glyphs.is_empty());
        assert!(
            layout
                .glyphs
                .windows(2)
                .any(|pair| pair[1].baseline_y > pair[0].baseline_y)
        );
        assert!(layout.glyphs.iter().all(|glyph| glyph.glyph_id == 0));
    }

    #[test]
    fn system_shaper_uses_real_glyph_ids_when_font_is_available() {
        let layout = SystemTextShaper.shape(TextParagraph::new("System shaping", 200.0));

        assert!(!layout.glyphs.is_empty());
        assert!(layout.glyphs.iter().any(|glyph| glyph.glyph_id != 0));
    }

    #[test]
    fn fallback_shaper_applies_alignment_offset_when_alignment_width_exists() {
        let mut paragraph = TextParagraph::new("center", 200.0);
        paragraph.text_align = Some(TextAlign::Center);
        paragraph.alignment_width = Some(100.0);

        let layout = FallbackTextShaper.shape(paragraph);

        assert_eq!(layout.metrics.width, 100.0);
        assert!(layout.glyphs.first().expect("first glyph").x > 0.0);
    }

    #[test]
    fn fallback_shaper_respects_max_lines_and_ellipsis() {
        let mut paragraph = TextParagraph::new("wrap me into one line only", 30.0);
        paragraph.max_lines = Some(1);
        paragraph.overflow = TextOverflow::Ellipsis;

        let layout = FallbackTextShaper.shape(paragraph);

        assert_eq!(layout.metrics.line_count, 1);
        assert_eq!(layout.glyphs.last().expect("ellipsis glyph").glyph, '…');
    }

    #[test]
    fn fallback_shaper_respects_soft_wrap_false() {
        let mut paragraph = TextParagraph::new("wrap me please", 30.0);
        paragraph.soft_wrap = false;

        let layout = FallbackTextShaper.shape(paragraph);

        assert_eq!(layout.metrics.line_count, 1);
        assert!(
            layout
                .glyphs
                .windows(2)
                .all(|pair| pair[1].baseline_y == pair[0].baseline_y)
        );
    }

    #[test]
    fn rustybuzz_feature_mapping_emits_tabular_numbers_feature() {
        let mut paragraph = TextParagraph::new("11", 100.0);
        paragraph.font.features.insert(FontFeature::TabularNumbers);

        let features = super::rustybuzz_features(&paragraph);

        assert_eq!(features.len(), 1);
    }
}
