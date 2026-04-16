use metal::MTLScissorRect;
use zeno_core::{Color, Rect, Transform2D};
use zeno_scene::{BlendMode, ClipRegion, DisplayList, Effect, StackingContextId};

use super::super::offscreen::CompositeParams;
use super::super::scissor::{intersect_scissor, scissor_for_rect};
use super::lookups::RenderLookupTables;

pub(super) fn apply_alpha(color: Color, opacity: f32) -> Color {
    let mut color = color;
    color.alpha = ((color.alpha as f32) * opacity.clamp(0.0, 1.0)).round() as u8;
    color
}

#[cfg(test)]
pub(super) fn clip_scissor(
    display_list: &DisplayList,
    clip_chain_id: zeno_scene::ClipChainId,
    parent_transform: Transform2D,
    viewport_width: f32,
    viewport_height: f32,
) -> MTLScissorRect {
    let render_lookups = RenderLookupTables::build(display_list);
    clip_scissor_with_lookups(
        display_list,
        &render_lookups,
        clip_chain_id,
        parent_transform,
        viewport_width,
        viewport_height,
    )
}

pub(super) fn clip_scissor_with_lookups(
    display_list: &DisplayList,
    render_lookups: &RenderLookupTables,
    clip_chain_id: zeno_scene::ClipChainId,
    parent_transform: Transform2D,
    viewport_width: f32,
    viewport_height: f32,
) -> MTLScissorRect {
    let mut scissor = scissor_for_rect(
        Rect::new(0.0, 0.0, viewport_width, viewport_height),
        viewport_width,
        viewport_height,
    );
    let mut current = render_lookups.clip_chain(display_list, clip_chain_id);
    while let Some(chain) = current {
        let rect = match &chain.clip {
            ClipRegion::Rect(rect) => *rect,
            ClipRegion::RoundedRect { rect, .. } => *rect,
        };
        // Apply the tile/root translation after the clip's world transform. See
        // display_list_renderer/item.rs for the reasoning.
        let transform =
            world_transform_with_lookups(render_lookups, chain.spatial_id).then(parent_transform);
        scissor = intersect_scissor(
            scissor,
            scissor_for_rect(transform.map_rect(rect), viewport_width, viewport_height),
        );
        current = chain
            .parent
            .and_then(|parent_id| render_lookups.clip_chain(display_list, parent_id));
    }
    scissor
}

#[cfg(test)]
#[allow(dead_code)]
pub(super) fn world_transform(
    display_list: &DisplayList,
    spatial_id: zeno_scene::SpatialNodeId,
) -> Transform2D {
    let render_lookups = RenderLookupTables::build(display_list);
    world_transform_with_lookups(&render_lookups, spatial_id)
}

pub(super) fn world_transform_with_lookups(
    render_lookups: &RenderLookupTables,
    spatial_id: zeno_scene::SpatialNodeId,
) -> Transform2D {
    render_lookups.world_transform(spatial_id)
}

#[cfg(test)]
pub(super) fn context_bounds(display_list: &DisplayList, context_id: StackingContextId) -> Rect {
    let render_lookups = RenderLookupTables::build(display_list);
    context_bounds_with_lookups(&render_lookups, context_id)
}

pub(super) fn context_bounds_with_lookups(
    render_lookups: &RenderLookupTables,
    context_id: StackingContextId,
) -> Rect {
    render_lookups.context_bounds(context_id)
}

pub(super) fn apply_effect_bounds(bounds: Rect, effects: &[Effect]) -> Rect {
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

pub(super) fn expand_rect(rect: Rect, amount: f32) -> Rect {
    Rect::new(
        rect.origin.x - amount,
        rect.origin.y - amount,
        rect.size.width + amount * 2.0,
        rect.size.height + amount * 2.0,
    )
}

pub(super) fn effect_sample_padding(effects: &[Effect]) -> f32 {
    let mut padding = 0.0f32;
    for effect in effects {
        match effect {
            Effect::Blur { sigma } => {
                padding = padding.max(sigma * 3.0);
            }
            Effect::DropShadow { dx, dy, blur, .. } => {
                padding = padding.max(dx.abs().max(dy.abs()) + blur * 3.0);
            }
        }
    }
    padding
}

pub(super) fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let left = a.origin.x.max(b.origin.x);
    let top = a.origin.y.max(b.origin.y);
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    (right > left && bottom > top).then(|| Rect::new(left, top, right - left, bottom - top))
}

pub(super) fn rect_contains_rect(container: Rect, rect: Rect) -> bool {
    rect.origin.x >= container.origin.x
        && rect.origin.y >= container.origin.y
        && rect.right() <= container.right()
        && rect.bottom() <= container.bottom()
}

pub(super) fn union_patch_rects(covered_rect: Rect, requested_rect: Rect) -> Vec<Rect> {
    let union_rect = covered_rect.union(&requested_rect);
    let mut patches = Vec::new();

    if union_rect.origin.x < covered_rect.origin.x {
        patches.push(Rect::new(
            union_rect.origin.x,
            union_rect.origin.y,
            covered_rect.origin.x - union_rect.origin.x,
            union_rect.size.height,
        ));
    }

    if union_rect.right() > covered_rect.right() {
        patches.push(Rect::new(
            covered_rect.right(),
            union_rect.origin.y,
            union_rect.right() - covered_rect.right(),
            union_rect.size.height,
        ));
    }

    let middle_left = union_rect.origin.x.max(covered_rect.origin.x);
    let middle_right = union_rect.right().min(covered_rect.right());
    let middle_width = (middle_right - middle_left).max(0.0);

    if middle_width > 0.0 && union_rect.origin.y < covered_rect.origin.y {
        patches.push(Rect::new(
            middle_left,
            union_rect.origin.y,
            middle_width,
            covered_rect.origin.y - union_rect.origin.y,
        ));
    }

    if middle_width > 0.0 && union_rect.bottom() > covered_rect.bottom() {
        patches.push(Rect::new(
            middle_left,
            covered_rect.bottom(),
            middle_width,
            union_rect.bottom() - covered_rect.bottom(),
        ));
    }

    patches
        .into_iter()
        .filter(|rect| rect.size.width > 0.0 && rect.size.height > 0.0)
        .collect()
}

pub(super) fn scene_blend_mode(mode: BlendMode) -> zeno_scene::SceneBlendMode {
    match mode {
        BlendMode::Normal => zeno_scene::SceneBlendMode::Normal,
        BlendMode::Multiply => zeno_scene::SceneBlendMode::Multiply,
        BlendMode::Screen => zeno_scene::SceneBlendMode::Screen,
    }
}

pub(super) fn composite_params_for_effects(
    effects: &[Effect],
    texture_width: f32,
    texture_height: f32,
) -> CompositeParams {
    let mut blur_sigma = 0.0;
    let mut shadow_blur = 0.0;
    let mut shadow_offset = [0.0, 0.0];
    let mut shadow_color = [0.0, 0.0, 0.0, 0.0];
    let mut flags = 0u32;
    for effect in effects {
        match effect {
            Effect::Blur { sigma } => {
                blur_sigma = *sigma;
                flags |= 1;
            }
            Effect::DropShadow {
                dx,
                dy,
                blur,
                color,
            } => {
                shadow_blur = *blur;
                shadow_offset = [*dx, *dy];
                shadow_color = [
                    f32::from(color.red) / 255.0,
                    f32::from(color.green) / 255.0,
                    f32::from(color.blue) / 255.0,
                    f32::from(color.alpha) / 255.0,
                ];
                flags |= 2;
            }
        }
    }
    CompositeParams {
        inv_texture_size: [1.0 / texture_width.max(1.0), 1.0 / texture_height.max(1.0)],
        blur_sigma,
        shadow_blur,
        shadow_offset,
        shadow_color,
        flags,
        _padding: [0, 0, 0],
    }
}
