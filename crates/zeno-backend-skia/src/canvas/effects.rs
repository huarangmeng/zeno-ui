use skia_safe as sk;
use zeno_core::Rect;
use zeno_graphics::{SceneBlendMode, SceneEffect, SceneLayer};

use crate::canvas::mapping::sk_color;

pub(crate) fn needs_save_layer(layer: &SceneLayer) -> bool {
    layer.offscreen
        || layer.opacity < 1.0
        || layer.blend_mode != SceneBlendMode::Normal
        || !layer.effects.is_empty()
}

pub(crate) fn layer_paint(layer: &SceneLayer) -> sk::Paint {
    let mut paint = sk::Paint::default();
    paint.set_anti_alias(true);
    paint.set_alpha_f(layer.opacity.clamp(0.0, 1.0));
    paint.set_blend_mode(sk_blend_mode(layer.blend_mode));
    if let Some(filter) = layer_image_filter(layer) {
        paint.set_image_filter(filter);
    }
    paint
}

fn sk_blend_mode(mode: SceneBlendMode) -> sk::BlendMode {
    match mode {
        SceneBlendMode::Normal => sk::BlendMode::SrcOver,
        SceneBlendMode::Multiply => sk::BlendMode::Multiply,
        SceneBlendMode::Screen => sk::BlendMode::Screen,
    }
}

fn layer_image_filter(layer: &SceneLayer) -> Option<sk::ImageFilter> {
    let mut current = None;
    for effect in &layer.effects {
        current = match effect {
            SceneEffect::Blur { sigma } => {
                sk::image_filters::blur((*sigma, *sigma), None, current, None)
            }
            SceneEffect::DropShadow {
                dx,
                dy,
                blur,
                color,
            } => sk::image_filters::drop_shadow(
                (*dx, *dy),
                (*blur, *blur),
                sk::Color4f::from(sk_color(*color)),
                None,
                current,
                None,
            ),
        };
    }
    current
}

pub(crate) fn layer_effect_bounds(layer: &SceneLayer) -> sk::Rect {
    let local_bounds = effect_bounds_for_scene_effects(layer.local_bounds, &layer.effects);
    sk::Rect::from_xywh(
        local_bounds.origin.x,
        local_bounds.origin.y,
        local_bounds.size.width,
        local_bounds.size.height,
    )
}

fn effect_bounds_for_scene_effects(bounds: Rect, effects: &[SceneEffect]) -> Rect {
    let mut visual_bounds = bounds;
    for effect in effects {
        match effect {
            SceneEffect::Blur { sigma } => {
                visual_bounds = expand_rect(visual_bounds, sigma * 3.0);
            }
            SceneEffect::DropShadow { dx, dy, blur, .. } => {
                let shadow_bounds = expand_rect(
                    Rect::new(
                        visual_bounds.origin.x + dx,
                        visual_bounds.origin.y + dy,
                        visual_bounds.size.width,
                        visual_bounds.size.height,
                    ),
                    blur * 3.0,
                );
                visual_bounds = visual_bounds.union(&shadow_bounds);
            }
        }
    }
    visual_bounds
}

fn expand_rect(rect: Rect, amount: f32) -> Rect {
    // Skia 在 save_layer 时会按 bounds 裁剪离屏内容，所以这里要把 blur/shadow 的可视外扩提前算进去。
    Rect::new(
        rect.origin.x - amount,
        rect.origin.y - amount,
        rect.size.width + amount * 2.0,
        rect.size.height + amount * 2.0,
    )
}
