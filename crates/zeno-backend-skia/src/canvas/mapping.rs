use skia_safe as sk;
use zeno_core::{Color, Transform2D};
use zeno_graphics::{SceneClip, Shape};

pub(crate) fn sk_color(color: Color) -> sk::Color {
    sk::Color::from_argb(color.alpha, color.red, color.green, color.blue)
}

pub(crate) fn apply_transform(canvas: &sk::Canvas, transform: Transform2D) {
    let matrix = sk::Matrix::new_all(
        transform.m11,
        transform.m21,
        transform.tx,
        transform.m12,
        transform.m22,
        transform.ty,
        0.0,
        0.0,
        1.0,
    );
    canvas.concat(&matrix);
}

pub(crate) fn apply_clip(canvas: &sk::Canvas, clip: SceneClip) {
    match clip {
        SceneClip::Rect(rect) => {
            canvas.clip_rect(
                sk::Rect::from_xywh(
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height,
                ),
                None,
                Some(true),
            );
        }
        SceneClip::RoundedRect { rect, radius } => {
            let rrect = sk::RRect::new_rect_xy(
                sk::Rect::from_xywh(
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height,
                ),
                radius,
                radius,
            );
            canvas.clip_rrect(rrect, None, Some(true));
        }
    }
}

pub(crate) fn draw_shape(canvas: &sk::Canvas, shape: &Shape, paint: &sk::Paint) {
    match shape {
        Shape::Rect(rect) => {
            let rect = sk::Rect::from_xywh(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
            );
            canvas.draw_rect(rect, paint);
        }
        Shape::RoundedRect { rect, radius } => {
            let rounded = sk::RRect::new_rect_xy(
                sk::Rect::from_xywh(
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height,
                ),
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
