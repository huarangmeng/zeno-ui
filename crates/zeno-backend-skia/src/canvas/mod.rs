mod draw;
mod effects;
mod layer;
mod mapping;
mod text;

use std::collections::HashSet;

use skia_safe as sk;
use zeno_core::{Color, Rect};
use zeno_scene::{
    ClipChainId, ClipRegion, DisplayItem, DisplayItemPayload, DisplayList, RetainedScene, Scene,
    SpatialNodeId,
};

pub use text::{SkiaTextCache, SkiaTextCacheStats};

use draw::draw_command;
use effects::{context_effect_bounds, context_paint, needs_save_layer_for_context};
use layer::render_retained_scene_layers;
use mapping::{apply_transform, sk_color};
use text::draw_text_layout;

pub fn render_scene_to_canvas(canvas: &sk::Canvas, scene: &Scene, text_cache: &mut SkiaTextCache) {
    let mut retained = RetainedScene::from_scene(scene.clone());
    render_retained_scene_to_canvas(canvas, &mut retained, text_cache);
}

pub fn render_scene_region_to_canvas(
    canvas: &sk::Canvas,
    scene: &Scene,
    dirty_rect: Rect,
    text_cache: &mut SkiaTextCache,
) {
    let clip = sk::Rect::from_xywh(
        dirty_rect.origin.x,
        dirty_rect.origin.y,
        dirty_rect.size.width,
        dirty_rect.size.height,
    );

    let mut retained = RetainedScene::from_scene(scene.clone());
    canvas.save();
    canvas.clip_rect(clip, None, Some(false));
    canvas.draw_rect(clip, &clear_paint(scene));
    render_retained_scene_layers(canvas, &mut retained, text_cache);
    canvas.restore();
}

fn clear_paint(scene: &Scene) -> sk::Paint {
    let mut paint = sk::Paint::default();
    paint.set_style(sk::paint::Style::Fill);
    paint.set_anti_alias(true);
    let clear = scene
        .clear_color
        .or_else(|| scene.clear_packet())
        .unwrap_or(Color::TRANSPARENT);
    paint.set_color(sk_color(clear));
    paint
}

pub fn render_retained_scene_to_canvas(
    canvas: &sk::Canvas,
    scene: &mut RetainedScene,
    text_cache: &mut SkiaTextCache,
) {
    if let Some(clear_color) = scene.clear_color {
        canvas.clear(sk_color(clear_color));
    }
    if scene.live_object_count() == 0 {
        for cmd in scene.packets() {
            draw_command(canvas, cmd, text_cache);
        }
        return;
    }
    render_retained_scene_layers(canvas, scene, text_cache);
}

pub fn render_retained_scene_region_to_canvas(
    canvas: &sk::Canvas,
    scene: &mut RetainedScene,
    dirty_rect: Rect,
    text_cache: &mut SkiaTextCache,
) {
    let clip = sk::Rect::from_xywh(
        dirty_rect.origin.x,
        dirty_rect.origin.y,
        dirty_rect.size.width,
        dirty_rect.size.height,
    );
    canvas.save();
    canvas.clip_rect(clip, None, Some(false));
    canvas.draw_rect(clip, &clear_paint_retained(scene));
    render_retained_scene_layers(canvas, scene, text_cache);
    canvas.restore();
}

fn clear_paint_retained(scene: &mut RetainedScene) -> sk::Paint {
    let mut paint = sk::Paint::default();
    paint.set_style(sk::paint::Style::Fill);
    paint.set_anti_alias(true);
    let clear = scene
        .clear_color
        .or_else(|| scene.clear_packet())
        .unwrap_or(Color::TRANSPARENT);
    paint.set_color(sk_color(clear));
    paint
}

pub fn render_display_list_to_canvas(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    text_cache: &mut SkiaTextCache,
) {
    canvas.clear(sk::Color::TRANSPARENT);
    render_display_list_scope(canvas, display_list, None, None, text_cache);
}

pub fn render_display_list_region_to_canvas(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    dirty_rect: Rect,
    text_cache: &mut SkiaTextCache,
) {
    let clip = sk::Rect::from_xywh(
        dirty_rect.origin.x,
        dirty_rect.origin.y,
        dirty_rect.size.width,
        dirty_rect.size.height,
    );
    canvas.save();
    canvas.clip_rect(clip, None, Some(false));
    let mut clear = sk::Paint::default();
    clear.set_style(sk::paint::Style::Fill);
    clear.set_color(sk::Color::TRANSPARENT);
    canvas.draw_rect(clip, &clear);
    render_display_list_scope(canvas, display_list, None, Some(dirty_rect), text_cache);
    canvas.restore();
}

fn render_display_list_scope(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    parent_context: Option<zeno_scene::StackingContextId>,
    dirty_rect: Option<Rect>,
    text_cache: &mut SkiaTextCache,
) {
    let mut rendered_child_contexts = HashSet::new();
    for item in &display_list.items {
        let Some(scope_entry) = scope_entry_for_item(display_list, parent_context, item) else {
            continue;
        };
        match scope_entry {
            ScopeEntry::Direct => {
                if dirty_rect.is_none_or(|dirty| item.visual_rect.intersects(&dirty)) {
                    render_display_item(canvas, display_list, item, text_cache);
                }
            }
            ScopeEntry::ChildContext(context_id) => {
                if !rendered_child_contexts.insert(context_id) {
                    continue;
                }
                if dirty_rect.is_some_and(|dirty| {
                    !stacking_context_bounds(display_list, context_id).intersects(&dirty)
                }) {
                    continue;
                }
                render_stacking_context(canvas, display_list, context_id, dirty_rect, text_cache);
            }
        }
    }
}

fn render_stacking_context(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    context_id: zeno_scene::StackingContextId,
    dirty_rect: Option<Rect>,
    text_cache: &mut SkiaTextCache,
) {
    let Some(context) = display_list
        .stacking_contexts
        .iter()
        .find(|context| context.id == context_id)
    else {
        return;
    };
    let initial_save_count = canvas.save_count();
    if needs_save_layer_for_context(context) {
        let bounds = context_effect_bounds(
            stacking_context_bounds(display_list, context_id),
            &context.effects,
        );
        let paint = context_paint(context);
        let layer_rec = sk::canvas::SaveLayerRec::default()
            .bounds(&bounds)
            .paint(&paint);
        canvas.save_layer(&layer_rec);
    } else {
        canvas.save();
    }
    render_display_list_scope(canvas, display_list, Some(context_id), dirty_rect, text_cache);
    canvas.restore_to_count(initial_save_count);
}

fn stacking_context_bounds(
    display_list: &DisplayList,
    context_id: zeno_scene::StackingContextId,
) -> Rect {
    let mut bounds: Option<Rect> = None;
    for item in &display_list.items {
        if item_in_stacking_context_subtree(display_list, item, context_id) {
            bounds = Some(match bounds {
                Some(current) => current.union(&item.visual_rect),
                None => item.visual_rect,
            });
        }
    }
    bounds.unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0))
}

fn item_in_stacking_context_subtree(
    display_list: &DisplayList,
    item: &DisplayItem,
    ancestor: zeno_scene::StackingContextId,
) -> bool {
    let mut current = item.stacking_context;
    while let Some(context_id) = current {
        if context_id == ancestor {
            return true;
        }
        current = parent_stacking_context(display_list, context_id);
    }
    false
}

fn parent_stacking_context(
    display_list: &DisplayList,
    context_id: zeno_scene::StackingContextId,
) -> Option<zeno_scene::StackingContextId> {
    let context = display_list
        .stacking_contexts
        .iter()
        .find(|context| context.id == context_id)?;
    let mut current = display_list
        .spatial_tree
        .nodes
        .iter()
        .find(|node| node.id == context.spatial_id)?
        .parent;
    while let Some(spatial_id) = current {
        if let Some(parent_context) = display_list
            .stacking_contexts
            .iter()
            .find(|candidate| candidate.spatial_id == spatial_id)
        {
            return Some(parent_context.id);
        }
        current = display_list
            .spatial_tree
            .nodes
            .iter()
            .find(|node| node.id == spatial_id)
            .and_then(|node| node.parent);
    }
    None
}

fn scope_entry_for_item(
    display_list: &DisplayList,
    parent_context: Option<zeno_scene::StackingContextId>,
    item: &DisplayItem,
) -> Option<ScopeEntry> {
    match (parent_context, item.stacking_context) {
        (None, None) => Some(ScopeEntry::Direct),
        (Some(parent), Some(current)) if current == parent => Some(ScopeEntry::Direct),
        (scope_parent, Some(current)) => immediate_child_context(display_list, scope_parent, current)
            .map(ScopeEntry::ChildContext),
        _ => None,
    }
}

fn immediate_child_context(
    display_list: &DisplayList,
    parent_context: Option<zeno_scene::StackingContextId>,
    mut current: zeno_scene::StackingContextId,
) -> Option<zeno_scene::StackingContextId> {
    let mut path = vec![current];
    while let Some(parent) = parent_stacking_context(display_list, current) {
        path.push(parent);
        current = parent;
    }
    path.reverse();
    match parent_context {
        None => path.first().copied(),
        Some(parent) => path
            .iter()
            .position(|&id| id == parent)
            .and_then(|index| path.get(index + 1).copied()),
    }
}

enum ScopeEntry {
    Direct,
    ChildContext(zeno_scene::StackingContextId),
}

fn render_display_item(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    item: &DisplayItem,
    text_cache: &mut SkiaTextCache,
) {
    canvas.save();
    apply_clip_chain(canvas, display_list, item.clip_chain_id);
    apply_spatial_transform(canvas, display_list, item.spatial_id);
    let Some(paint) = paint_for_item(item) else {
        canvas.restore();
        return;
    };
    match &item.payload {
        DisplayItemPayload::FillRect { rect, .. } => {
            let rect = sk::Rect::from_xywh(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
            );
            canvas.draw_rect(rect, &paint);
        }
        DisplayItemPayload::FillRoundedRect { rect, radius, .. } => {
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
            canvas.draw_rrect(rounded, &paint);
        }
        DisplayItemPayload::TextRun(text) => {
            draw_text_layout(canvas, text.position, &text.layout, text.color, text_cache);
        }
        DisplayItemPayload::Image(image) => {
            let info = sk::ImageInfo::new(
                (image.width as i32, image.height as i32),
                sk::ColorType::RGBA8888,
                sk::AlphaType::Premul,
                None,
            );
            if let Some(sk_image) =
                sk::images::raster_from_data(&info, sk::Data::new_copy(&image.rgba8), (image.width * 4) as usize)
            {
                let dst = sk::Rect::from_xywh(
                    image.dest_rect.origin.x,
                    image.dest_rect.origin.y,
                    image.dest_rect.size.width,
                    image.dest_rect.size.height,
                );
                canvas.draw_image_rect(sk_image, None, &dst, &paint);
            }
        }
        DisplayItemPayload::Custom => {}
    }
    canvas.restore();
}

fn paint_for_item(item: &DisplayItem) -> Option<sk::Paint> {
    let mut paint = sk::Paint::default();
    paint.set_style(sk::paint::Style::Fill);
    paint.set_anti_alias(true);
    let base_color = match &item.payload {
        DisplayItemPayload::FillRect { color, .. } => *color,
        DisplayItemPayload::FillRoundedRect { color, .. } => *color,
        _ => return None,
    };
    paint.set_color(sk_color(base_color));
    Some(paint)
}

fn apply_spatial_transform(canvas: &sk::Canvas, display_list: &DisplayList, spatial_id: SpatialNodeId) {
    let Some(node) = display_list
        .spatial_tree
        .nodes
        .iter()
        .find(|node| node.id == spatial_id)
    else {
        return;
    };
    apply_transform(canvas, node.world_transform);
}

fn apply_clip_chain(canvas: &sk::Canvas, display_list: &DisplayList, clip_chain_id: ClipChainId) {
    let mut chain = Vec::new();
    let mut current = display_list
        .clip_chains
        .chains
        .iter()
        .find(|chain| chain.id == clip_chain_id);
    while let Some(entry) = current {
        chain.push(entry);
        current = entry.parent.and_then(|parent_id| {
            display_list
                .clip_chains
                .chains
                .iter()
                .find(|candidate| candidate.id == parent_id)
        });
    }
    for entry in chain.into_iter().rev() {
        let mut path = clip_region_path(&entry.clip);
        if let Some(node) = display_list
            .spatial_tree
            .nodes
            .iter()
            .find(|node| node.id == entry.spatial_id)
        {
            path = path.make_transform(&matrix_for_transform(node.world_transform));
        }
        canvas.clip_path(&path, None, Some(true));
    }
}

fn clip_region_path(region: &ClipRegion) -> sk::Path {
    let mut builder = sk::PathBuilder::new();
    match region {
        ClipRegion::Rect(rect) => {
            builder.add_rect(
                sk::Rect::from_xywh(
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height,
                ),
                None,
                None,
            );
        }
        ClipRegion::RoundedRect { rect, radius } => {
            builder.add_rrect(
                sk::RRect::new_rect_xy(
                    sk::Rect::from_xywh(
                        rect.origin.x,
                        rect.origin.y,
                        rect.size.width,
                        rect.size.height,
                    ),
                    *radius,
                    *radius,
                ),
                None,
                None,
            );
        }
    }
    builder.detach()
}

fn matrix_for_transform(transform: zeno_core::Transform2D) -> sk::Matrix {
    sk::Matrix::new_all(
        transform.m11,
        transform.m21,
        transform.tx,
        transform.m12,
        transform.m22,
        transform.ty,
        0.0,
        0.0,
        1.0,
    )
}
