mod effects;
mod image;
mod mapping;
mod text;

use std::collections::HashSet;

use skia_safe as sk;
use zeno_core::Rect;
use zeno_scene::{
    ClipChainId, ClipRegion, CompositorLayerId, CompositorLayerTree, CompositorScopeEntry,
    DisplayItem, DisplayItemPayload, DisplayList, SpatialNodeId,
};

pub use image::{SkiaImageCache, SkiaImageCacheStats};
pub use text::{SkiaTextCache, SkiaTextCacheStats};

use effects::{context_effect_bounds, context_paint, needs_save_layer_for_context};
use mapping::{apply_transform, sk_color};
use text::draw_text_layout;

pub fn render_display_list_to_canvas(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    text_cache: &mut SkiaTextCache,
    image_cache: &mut SkiaImageCache,
) {
    canvas.clear(sk::Color::TRANSPARENT);
    render_display_list_scope(
        canvas,
        display_list,
        None,
        None,
        None,
        text_cache,
        image_cache,
    );
}

pub fn render_display_list_region_to_canvas(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    dirty_rect: Rect,
    text_cache: &mut SkiaTextCache,
    image_cache: &mut SkiaImageCache,
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
    render_display_list_scope(
        canvas,
        display_list,
        None,
        None,
        Some(dirty_rect),
        text_cache,
        image_cache,
    );
    canvas.restore();
}

pub fn render_display_list_tile_to_canvas(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    layer_tree: &CompositorLayerTree,
    tile_rect: Rect,
    text_cache: &mut SkiaTextCache,
    image_cache: &mut SkiaImageCache,
) {
    let local_clip = sk::Rect::from_xywh(0.0, 0.0, tile_rect.size.width, tile_rect.size.height);
    canvas.clear(sk::Color::TRANSPARENT);
    canvas.save();
    canvas.clip_rect(local_clip, None, Some(false));
    canvas.translate((-tile_rect.origin.x, -tile_rect.origin.y));
    render_display_list_scope(
        canvas,
        display_list,
        Some(layer_tree),
        None,
        Some(tile_rect),
        text_cache,
        image_cache,
    );
    canvas.restore();
}

fn render_display_list_scope(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    layer_tree: Option<&CompositorLayerTree>,
    parent_context: Option<zeno_scene::StackingContextId>,
    dirty_rect: Option<Rect>,
    text_cache: &mut SkiaTextCache,
    image_cache: &mut SkiaImageCache,
) {
    for step in scope_steps(display_list, layer_tree, parent_context) {
        match step {
            ScopeStep::Direct(item_index) => {
                let Some(item) = display_list.items.get(item_index) else {
                    continue;
                };
                let should_render = match dirty_rect {
                    Some(dirty) => item.visual_rect.intersects(&dirty),
                    None => true,
                };
                if should_render {
                    render_display_item(canvas, display_list, item, text_cache, image_cache);
                }
            }
            ScopeStep::ChildContext(context_id) => {
                let context_dirty = match dirty_rect {
                    Some(dirty) => {
                        stacking_context_bounds(display_list, context_id).intersects(&dirty)
                    }
                    None => true,
                };
                if !context_dirty {
                    continue;
                }
                render_stacking_context(
                    canvas,
                    display_list,
                    layer_tree,
                    context_id,
                    dirty_rect,
                    text_cache,
                    image_cache,
                );
            }
        }
    }
}

fn render_stacking_context(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    layer_tree: Option<&CompositorLayerTree>,
    context_id: zeno_scene::StackingContextId,
    dirty_rect: Option<Rect>,
    text_cache: &mut SkiaTextCache,
    image_cache: &mut SkiaImageCache,
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
    render_display_list_scope(
        canvas,
        display_list,
        layer_tree,
        Some(context_id),
        dirty_rect,
        text_cache,
        image_cache,
    );
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
    display_list
        .stacking_contexts
        .iter()
        .find(|context| context.id == context_id)
        .and_then(|context| context.parent)
}

fn scope_entry_for_item(
    display_list: &DisplayList,
    parent_context: Option<zeno_scene::StackingContextId>,
    item: &DisplayItem,
) -> Option<ScopeEntry> {
    match (parent_context, item.stacking_context) {
        (None, None) => Some(ScopeEntry::Direct),
        (Some(parent), Some(current)) if current == parent => Some(ScopeEntry::Direct),
        (scope_parent, Some(current)) => {
            immediate_child_context(display_list, scope_parent, current)
                .map(ScopeEntry::ChildContext)
        }
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

enum ScopeStep {
    Direct(usize),
    ChildContext(zeno_scene::StackingContextId),
}

fn scope_steps(
    display_list: &DisplayList,
    layer_tree: Option<&CompositorLayerTree>,
    parent_context: Option<zeno_scene::StackingContextId>,
) -> Vec<ScopeStep> {
    if let Some(steps) =
        layer_tree.and_then(|tree| scope_steps_from_layer_tree(display_list, tree, parent_context))
    {
        return steps;
    }
    let mut rendered_child_contexts = HashSet::new();
    let mut steps = Vec::new();
    for (item_index, item) in display_list.items.iter().enumerate() {
        let Some(scope_entry) = scope_entry_for_item(display_list, parent_context, item) else {
            continue;
        };
        match scope_entry {
            ScopeEntry::Direct => steps.push(ScopeStep::Direct(item_index)),
            ScopeEntry::ChildContext(context_id) => {
                if rendered_child_contexts.insert(context_id) {
                    steps.push(ScopeStep::ChildContext(context_id));
                }
            }
        }
    }
    steps
}

fn scope_steps_from_layer_tree(
    display_list: &DisplayList,
    layer_tree: &CompositorLayerTree,
    parent_context: Option<zeno_scene::StackingContextId>,
) -> Option<Vec<ScopeStep>> {
    let layer = layer_for_scope(display_list, layer_tree, parent_context)?;
    Some(
        layer
            .scope_entries
            .iter()
            .filter_map(|entry| match entry {
                CompositorScopeEntry::DirectItem(item_index) => {
                    Some(ScopeStep::Direct(*item_index))
                }
                CompositorScopeEntry::ChildLayer(layer_id) => {
                    child_context_id_for_layer(display_list, layer_tree, *layer_id)
                        .map(ScopeStep::ChildContext)
                }
            })
            .collect(),
    )
}

fn layer_for_scope<'a>(
    display_list: &'a DisplayList,
    layer_tree: &'a CompositorLayerTree,
    parent_context: Option<zeno_scene::StackingContextId>,
) -> Option<&'a zeno_scene::CompositorLayer> {
    match parent_context {
        None => layer_tree
            .layers
            .iter()
            .find(|layer| layer.layer_id == CompositorLayerId(0)),
        Some(context_id) => {
            let context_index = display_list
                .stacking_contexts
                .iter()
                .position(|context| context.id == context_id)?;
            layer_tree
                .layers
                .iter()
                .find(|layer| layer.stacking_context_index == Some(context_index))
        }
    }
}

fn child_context_id_for_layer(
    display_list: &DisplayList,
    layer_tree: &CompositorLayerTree,
    layer_id: CompositorLayerId,
) -> Option<zeno_scene::StackingContextId> {
    let layer = layer_tree
        .layers
        .iter()
        .find(|layer| layer.layer_id == layer_id)?;
    let context_index = layer.stacking_context_index?;
    display_list
        .stacking_contexts
        .get(context_index)
        .map(|context| context.id)
}

fn render_display_item(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    item: &DisplayItem,
    text_cache: &mut SkiaTextCache,
    image_cache: &mut SkiaImageCache,
) {
    canvas.save();
    apply_clip_chain(canvas, display_list, item.clip_chain_id);
    apply_spatial_transform(canvas, display_list, item.spatial_id);
    match &item.payload {
        DisplayItemPayload::FillRect { rect, .. } => {
            let Some(paint) = paint_for_item(item) else {
                canvas.restore();
                return;
            };
            let rect = sk::Rect::from_xywh(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
            );
            canvas.draw_rect(rect, &paint);
        }
        DisplayItemPayload::FillRoundedRect { rect, radius, .. } => {
            let Some(paint) = paint_for_item(item) else {
                canvas.restore();
                return;
            };
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
            if let Some(sk_image) =
                image_cache.resolve_rgba8(image.cache_key, image.width, image.height, &image.rgba8)
            {
                let dst = sk::Rect::from_xywh(
                    image.dest_rect.origin.x,
                    image.dest_rect.origin.y,
                    image.dest_rect.size.width,
                    image.dest_rect.size.height,
                );
                let paint = sk::Paint::default();
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

fn apply_spatial_transform(
    canvas: &sk::Canvas,
    display_list: &DisplayList,
    spatial_id: SpatialNodeId,
) {
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
