use metal::MTLScissorRect;
use zeno_core::Rect;
#[cfg(test)]
use zeno_core::Transform2D;

// 统一管理裁剪矩形与坐标变换，避免渲染递归里散落几何细节。
pub(super) fn scissor_for_rect(
    rect: Rect,
    viewport_width: f32,
    viewport_height: f32,
) -> MTLScissorRect {
    let min_x = rect.origin.x.max(0.0).floor() as u64;
    let min_y = rect.origin.y.max(0.0).floor() as u64;
    let max_x = (rect.origin.x + rect.size.width)
        .min(viewport_width)
        .max(min_x as f32)
        .ceil() as u64;
    let max_y = (rect.origin.y + rect.size.height)
        .min(viewport_height)
        .max(min_y as f32)
        .ceil() as u64;
    MTLScissorRect {
        x: min_x,
        y: min_y,
        width: max_x.saturating_sub(min_x),
        height: max_y.saturating_sub(min_y),
    }
}

pub(super) fn effective_root_scissor(
    dirty_bounds: Option<Rect>,
    viewport_width: f32,
    viewport_height: f32,
) -> MTLScissorRect {
    dirty_bounds.map_or_else(
        || {
            scissor_for_rect(
                Rect::new(0.0, 0.0, viewport_width, viewport_height),
                viewport_width,
                viewport_height,
            )
        },
        |bounds| scissor_for_rect(bounds, viewport_width, viewport_height),
    )
}

pub(super) fn intersect_scissor(a: MTLScissorRect, b: MTLScissorRect) -> MTLScissorRect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    MTLScissorRect {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}

#[cfg(test)]
pub(super) fn rect_from_scissor(scissor: MTLScissorRect) -> Rect {
    Rect::new(
        scissor.x as f32,
        scissor.y as f32,
        scissor.width as f32,
        scissor.height as f32,
    )
}

#[cfg(test)]
pub(super) fn inverse_map_rect(transform: Transform2D, rect: Rect) -> Option<Rect> {
    let determinant = transform.m11 * transform.m22 - transform.m12 * transform.m21;
    if determinant.abs() <= f32::EPSILON {
        return None;
    }
    let inverse = Transform2D {
        m11: transform.m22 / determinant,
        m12: -transform.m12 / determinant,
        m21: -transform.m21 / determinant,
        m22: transform.m11 / determinant,
        tx: (transform.m21 * transform.ty - transform.m22 * transform.tx) / determinant,
        ty: (transform.m12 * transform.tx - transform.m11 * transform.ty) / determinant,
    };
    Some(inverse.map_rect(rect))
}
