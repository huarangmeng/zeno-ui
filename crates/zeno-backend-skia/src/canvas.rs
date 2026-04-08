use skia_safe as sk;
use zeno_core::Color;
use zeno_graphics::{DrawCommand, Scene, Shape};

pub fn render_scene_to_canvas(canvas: &sk::Canvas, scene: &Scene) {
    for cmd in &scene.commands {
        match cmd {
            DrawCommand::Clear(color) => {
                canvas.clear(sk_color(*color));
            }
            DrawCommand::Fill { shape, brush } => {
                let mut paint = sk::Paint::default();
                paint.set_style(skia_safe::paint::Style::Fill);
                paint.set_anti_alias(true);
                let zeno_graphics::Brush::Solid(c) = brush;
                paint.set_color(sk_color(*c));
                draw_shape(canvas, shape, &paint);
            }
            DrawCommand::Stroke { shape, stroke } => {
                let mut paint = sk::Paint::default();
                paint.set_style(skia_safe::paint::Style::Stroke);
                paint.set_anti_alias(true);
                paint.set_stroke_width(stroke.width);
                paint.set_color(sk_color(stroke.color));
                draw_shape(canvas, shape, &paint);
            }
            DrawCommand::Text { position, layout, color } => {
                let mut paint = sk::Paint::default();
                paint.set_anti_alias(true);
                paint.set_color(sk_color(*color));
                let mut font = match resolve_typeface(&layout.paragraph.font.family) {
                    Some(typeface) => {
                        sk::Font::from_typeface(typeface, layout.paragraph.font_size.max(12.0))
                    }
                    None => {
                        let mut font = sk::Font::default();
                        font.set_size(layout.paragraph.font_size.max(12.0));
                        font
                    }
                };
                font.set_edging(sk::font::Edging::AntiAlias);
                canvas.draw_str(layout.paragraph.text.as_str(), (position.x, position.y), &font, &paint);
            }
        }
    }
}

pub fn sk_color(color: Color) -> sk::Color {
    sk::Color::from_argb(color.alpha, color.red, color.green, color.blue)
}

fn draw_shape(canvas: &sk::Canvas, shape: &Shape, paint: &sk::Paint) {
    match shape {
        Shape::Rect(rect) => {
            let rect = sk::Rect::from_xywh(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height);
            canvas.draw_rect(rect, paint);
        }
        Shape::RoundedRect { rect, radius } => {
            let rounded = sk::RRect::new_rect_xy(
                sk::Rect::from_xywh(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height),
                *radius,
                *radius,
            );
            canvas.draw_rrect(rounded, paint);
        }
        Shape::Circle { center, radius } => {
            canvas.draw_circle((center.x, center.y), *radius, paint);
        }
    }
}

fn resolve_typeface(requested_family: &str) -> Option<sk::Typeface> {
    let font_mgr = sk::FontMgr::default();
    let mut families = vec![requested_family, "PingFang SC", "Helvetica Neue", "Arial", "Noto Sans"];
    families.retain(|family| !family.is_empty() && *family != "System");

    for family in families {
        if let Some(typeface) = font_mgr.match_family_style(family, sk::FontStyle::normal()) {
            return Some(typeface);
        }
    }

    None
}
