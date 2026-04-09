use skia_safe as sk;
use zeno_scene::DrawCommand;

use crate::canvas::{
    mapping::{draw_shape, sk_color},
    text::SkiaTextCache,
};

pub(crate) fn draw_command(canvas: &sk::Canvas, cmd: &DrawCommand, text_cache: &mut SkiaTextCache) {
    match cmd {
        DrawCommand::Clear(color) => {
            canvas.clear(sk_color(*color));
        }
        DrawCommand::Fill { shape, brush } => {
            let mut paint = sk::Paint::default();
            paint.set_style(sk::paint::Style::Fill);
            paint.set_anti_alias(true);
            let zeno_scene::Brush::Solid(color) = brush;
            paint.set_color(sk_color(*color));
            draw_shape(canvas, shape, &paint);
        }
        DrawCommand::Stroke { shape, stroke } => {
            let mut paint = sk::Paint::default();
            paint.set_style(sk::paint::Style::Stroke);
            paint.set_anti_alias(true);
            paint.set_stroke_width(stroke.width);
            paint.set_color(sk_color(stroke.color));
            draw_shape(canvas, shape, &paint);
        }
        DrawCommand::Text {
            position,
            layout,
            color,
        } => {
            let mut paint = sk::Paint::default();
            paint.set_anti_alias(true);
            paint.set_color(sk_color(*color));
            let mut font = text_cache.resolve_font(
                cmd.resource_key(),
                &layout.paragraph.font.family,
                layout.paragraph.font_size.max(12.0),
            );
            font.set_edging(sk::font::Edging::AntiAlias);
            let mut glyph_run = Vec::new();
            for glyph in &layout.glyphs {
                if glyph.glyph_id != 0 {
                    glyph_run.push(glyph);
                    continue;
                }
                flush_glyph_run(canvas, &glyph_run, *position, &font, &paint);
                glyph_run.clear();
                canvas.draw_str(
                    glyph.glyph.to_string(),
                    (position.x + glyph.x, position.y + glyph.baseline_y),
                    &font,
                    &paint,
                );
            }
            flush_glyph_run(canvas, &glyph_run, *position, &font, &paint);
        }
    }
}

fn flush_glyph_run(
    canvas: &sk::Canvas,
    glyph_run: &[&zeno_text::ShapedGlyph],
    position: zeno_core::Point,
    font: &sk::Font,
    paint: &sk::Paint,
) {
    if glyph_run.is_empty() {
        return;
    }
    let glyph_ids: Vec<u16> = glyph_run.iter().map(|glyph| glyph.glyph_id).collect();
    let positions: Vec<sk::Point> = glyph_run
        .iter()
        .map(|glyph| sk::Point::new(glyph.x, glyph.baseline_y))
        .collect();
    canvas.draw_glyphs_at(
        &glyph_ids,
        positions.as_slice(),
        sk::Point::new(position.x, position.y),
        font,
        paint,
    );
}
