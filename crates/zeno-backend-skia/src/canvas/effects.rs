use skia_safe as sk;
use zeno_core::Rect;
use zeno_scene::{BlendMode, Effect, StackingContext};

use crate::canvas::mapping::sk_color;

pub(crate) fn needs_save_layer_for_context(context: &StackingContext) -> bool {
    context.needs_offscreen
        || context.opacity < 1.0
        || context.blend_mode != BlendMode::Normal
        || !context.effects.is_empty()
}

pub(crate) fn context_paint(context: &StackingContext) -> sk::Paint {
    let mut paint = sk::Paint::default();
    paint.set_anti_alias(true);
    paint.set_alpha_f(context.opacity.clamp(0.0, 1.0));
    paint.set_blend_mode(sk_blend_mode_display(context.blend_mode));
    if let Some(filter) = display_image_filter(&context.effects) {
        paint.set_image_filter(filter);
    }
    paint
}

pub(crate) fn context_effect_bounds(bounds: Rect, effects: &[Effect]) -> sk::Rect {
    rect_to_sk(effect_bounds_for_display_effects(bounds, effects))
}

fn rect_to_sk(bounds: Rect) -> sk::Rect {
    sk::Rect::from_xywh(
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
    )
}

fn effect_bounds_for_display_effects(bounds: Rect, effects: &[Effect]) -> Rect {
    let mut visual_bounds = bounds;
    for effect in effects {
        match effect {
            Effect::Blur { sigma } => {
                visual_bounds = expand_rect(visual_bounds, sigma * 3.0);
            }
            Effect::DropShadow { dx, dy, blur, .. } => {
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

fn sk_blend_mode_display(mode: BlendMode) -> sk::BlendMode {
    match mode {
        BlendMode::Normal => sk::BlendMode::SrcOver,
        BlendMode::Multiply => sk::BlendMode::Multiply,
        BlendMode::Screen => sk::BlendMode::Screen,
    }
}

fn display_image_filter(effects: &[Effect]) -> Option<sk::ImageFilter> {
    let mut current = None;
    for effect in effects {
        current = match effect {
            Effect::Blur { sigma } => {
                sk::image_filters::blur((*sigma, *sigma), None, current, None)
            }
            Effect::DropShadow {
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

fn expand_rect(rect: Rect, amount: f32) -> Rect {
    // Skia 在 save_layer 时会按 bounds 裁剪离屏内容，所以这里要把 blur/shadow 的可视外扩提前算进去。
    Rect::new(
        rect.origin.x - amount,
        rect.origin.y - amount,
        rect.size.width + amount * 2.0,
        rect.size.height + amount * 2.0,
    )
}
