use skia_safe as sk;
use zeno_core::{Color, Transform2D};

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
