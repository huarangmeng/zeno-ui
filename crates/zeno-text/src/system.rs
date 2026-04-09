use crate::{TextCapabilities, TextLayout, TextMetrics, TextParagraph};

pub trait TextSystem: Send + Sync {
    fn name(&self) -> &'static str;

    fn capabilities(&self) -> TextCapabilities;

    fn layout(&self, paragraph: TextParagraph) -> TextLayout;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FallbackTextSystem;

impl TextSystem for FallbackTextSystem {
    fn name(&self) -> &'static str {
        "fallback-text"
    }

    fn capabilities(&self) -> TextCapabilities {
        TextCapabilities::minimal()
    }

    fn layout(&self, paragraph: TextParagraph) -> TextLayout {
        let average_advance = paragraph.font_size * 0.55;
        let width = paragraph
            .text
            .chars()
            .count()
            .min((paragraph.max_width / average_advance).floor() as usize)
            as f32
            * average_advance;
        let measured_width = if paragraph.max_width <= 0.0 {
            0.0
        } else {
            width.min(paragraph.max_width)
        };
        let line_count = if paragraph.max_width <= 0.0 {
            0
        } else {
            let estimated_total_width = paragraph.text.chars().count() as f32 * average_advance;
            (estimated_total_width / paragraph.max_width).ceil().max(1.0) as usize
        };
        let ascent = paragraph.font_size * 0.8;
        let descent = paragraph.font_size * 0.2;
        let metrics = TextMetrics {
            width: measured_width,
            height: paragraph.font_size * 1.4 * line_count as f32,
            line_count,
            ascent,
            descent,
        };
        TextLayout { paragraph, metrics }
    }
}
