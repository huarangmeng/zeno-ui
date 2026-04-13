use skia_safe as sk;
use zeno_scene::DrawCommand;

use crate::canvas::{
    mapping::{draw_shape, sk_color},
    text::{SkiaTextCache, draw_text_layout},
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
            let _ = cmd;
            draw_text_layout(canvas, *position, layout, *color, text_cache);
        }
    }
}
