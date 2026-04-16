use zeno_core::{Point, Rect, Size, Transform2D};
use zeno_scene::{
    ClipChain, ClipChainId, ClipChainStore, ClipRegion, DisplayImage, DisplayItem, DisplayItemId,
    DisplayItemPayload, DisplayList, DisplayTextRun, Effect, ImageCacheKey, RetainedDisplayList,
    SpatialNode, SpatialNodeId, SpatialTree, StackingContext, StackingContextId,
};

use super::*;
use crate::frontend::{
    FrontendObject, FrontendObjectKind, FrontendObjectTable, compile_object_table,
};
use crate::image::ImageResourceTable;
use crate::layout::LayoutArena;
use crate::modifier::ClipMode;

pub(super) fn build_retained_display_list(
    root: &Node,
    layout: &LayoutArena,
    image_resources: &ImageResourceTable,
    viewport: Size,
) -> RetainedDisplayList {
    let frontend = compile_object_table(root);
    build_retained_display_list_from_frontend(&frontend, layout, image_resources, viewport)
}

#[must_use]
pub(super) fn snapshot_display_list(retained: &RetainedDisplayList, viewport: Size) -> DisplayList {
    retained.snapshot(viewport)
}

fn build_retained_display_list_from_frontend(
    objects: &FrontendObjectTable,
    layout: &LayoutArena,
    image_resources: &ImageResourceTable,
    viewport: Size,
) -> RetainedDisplayList {
    let mut list = RetainedDisplayList::new(viewport);
    let stacking_context_map = build_stacking_context_map(objects);
    list.spatial_tree = SpatialTree {
        nodes: build_spatial_tree(objects, layout),
    };
    list.clip_chains = ClipChainStore {
        chains: build_clip_chains(objects, layout, viewport),
    };
    list.stacking_contexts = build_stacking_contexts(objects, &stacking_context_map);
    for index in 0..objects.len() {
        list.replace_object_items(
            index,
            items_for_object(
                objects,
                index,
                layout,
                &list.spatial_tree,
                image_resources,
                stacking_context_map[index],
            ),
        );
    }
    list.compact_if_needed();

    list
}

fn build_stacking_context_map(objects: &FrontendObjectTable) -> Vec<Option<StackingContextId>> {
    let mut map = vec![None; objects.len()];
    for index in 0..objects.len() {
        let object = objects.object(index);
        map[index] = if object_creates_stacking_context(&object.style) {
            Some(StackingContextId(index as u32))
        } else {
            objects
                .parent_index_of(index)
                .and_then(|parent| map[parent])
        };
    }
    map
}

fn build_spatial_tree(objects: &FrontendObjectTable, layout: &LayoutArena) -> Vec<SpatialNode> {
    let mut nodes: Vec<SpatialNode> = Vec::with_capacity(objects.len());
    for index in 0..objects.len() {
        let slot = layout.slot_at(index);
        let object = objects.object(index);
        let parent_origin = objects
            .parent_index_of(index)
            .map(|parent| layout.slot_at(parent).frame.origin)
            .unwrap_or(Point::new(0.0, 0.0));
        let size = slot.frame.size;
        let pivot = Point::new(
            size.width * object.style.transform_origin.x,
            size.height * object.style.transform_origin.y,
        );
        let local_transform = Transform2D::translation(-pivot.x, -pivot.y)
            .then(object.style.transform)
            .then(Transform2D::translation(pivot.x, pivot.y))
            .then(Transform2D::translation(
                slot.frame.origin.x - parent_origin.x,
                slot.frame.origin.y - parent_origin.y,
            ));
        // For a point in local space, we apply the node's local transform first, then its parent
        // world transform to reach scene space.
        let world_transform = objects
            .parent_index_of(index)
            .map(|parent| local_transform.then(nodes[parent].world_transform))
            .unwrap_or(local_transform);
        nodes.push(SpatialNode {
            id: SpatialNodeId(index as u32),
            parent: objects
                .parent_index_of(index)
                .map(|idx| SpatialNodeId(idx as u32)),
            local_transform,
            world_transform,
            dirty: false,
        });
    }
    nodes
}

fn build_clip_chains(
    objects: &FrontendObjectTable,
    layout: &LayoutArena,
    viewport: Size,
) -> Vec<ClipChain> {
    let mut chains = vec![ClipChain {
        id: ClipChainId(0),
        spatial_id: SpatialNodeId(0),
        clip: ClipRegion::Rect(Rect::new(0.0, 0.0, viewport.width, viewport.height)),
        parent: None,
    }];
    for index in 0..objects.len() {
        let object = objects.object(index);
        if object.style.clip.is_some() {
            let slot = layout.slot_at(index);
            chains.push(ClipChain {
                id: ClipChainId(index as u32 + 1),
                spatial_id: SpatialNodeId(index as u32),
                clip: match object.style.clip {
                    Some(ClipMode::Bounds) => ClipRegion::Rect(Rect::new(
                        0.0,
                        0.0,
                        slot.frame.size.width,
                        slot.frame.size.height,
                    )),
                    Some(ClipMode::RoundedBounds { radius }) => ClipRegion::RoundedRect {
                        rect: Rect::new(0.0, 0.0, slot.frame.size.width, slot.frame.size.height),
                        radius,
                    },
                    None => unreachable!("clip chain only created when clip mode is present"),
                },
                parent: Some(ClipChainId(0)),
            });
        }
    }
    chains
}

fn build_stacking_contexts(
    objects: &FrontendObjectTable,
    stacking_context_map: &[Option<StackingContextId>],
) -> Vec<StackingContext> {
    let mut contexts = Vec::new();
    for index in 0..objects.len() {
        let object = objects.object(index);
        if !object_creates_stacking_context(&object.style) {
            continue;
        }
        let effects = effects_for_object(object);
        contexts.push(StackingContext {
            id: StackingContextId(index as u32),
            parent: objects
                .parent_index_of(index)
                .and_then(|parent_index| stacking_context_map[parent_index]),
            paint_order: index + 1,
            spatial_id: SpatialNodeId(index as u32),
            opacity: object.style.opacity,
            blend_mode: match object.style.blend_mode {
                BlendMode::Normal => zeno_scene::BlendMode::Normal,
                BlendMode::Multiply => zeno_scene::BlendMode::Multiply,
                BlendMode::Screen => zeno_scene::BlendMode::Screen,
            },
            effects,
            needs_offscreen: object.style.layer
                || object.style.opacity < 1.0
                || object.style.blend_mode != BlendMode::Normal
                || object.style.blur.is_some()
                || object.style.drop_shadow.is_some(),
        });
    }
    contexts
}

fn items_for_object(
    objects: &FrontendObjectTable,
    index: usize,
    layout: &LayoutArena,
    spatial_tree: &SpatialTree,
    image_resources: &ImageResourceTable,
    stacking_context: Option<StackingContextId>,
) -> Vec<DisplayItem> {
    let object = objects.object(index);
    let slot = layout.slot_at(index);
    let clip_chain_id = if object.style.clip.is_some() {
        ClipChainId(index as u32 + 1)
    } else {
        ClipChainId(0)
    };
    let local_visual_rect = Rect::new(0.0, 0.0, slot.frame.size.width, slot.frame.size.height);
    let visual_rect = spatial_tree
        .nodes
        .get(index)
        .map(|node| node.world_transform.map_rect(local_visual_rect))
        .unwrap_or(slot.frame);
    let mut items = Vec::new();
    if let Some(background) = object.style.background {
        items.push(DisplayItem {
            item_id: DisplayItemId((index as u32) * 2),
            spatial_id: SpatialNodeId(index as u32),
            clip_chain_id,
            stacking_context,
            visual_rect,
            payload: if object.style.corner_radius > 0.0 {
                DisplayItemPayload::FillRoundedRect {
                    rect: Rect::new(0.0, 0.0, slot.frame.size.width, slot.frame.size.height),
                    radius: object.style.corner_radius,
                    color: background,
                }
            } else {
                DisplayItemPayload::FillRect {
                    rect: Rect::new(0.0, 0.0, slot.frame.size.width, slot.frame.size.height),
                    color: background,
                }
            },
        });
    }
    if matches!(&object.kind, FrontendObjectKind::Text(_)) {
        let text_layout = slot
            .text_layout
            .as_ref()
            .expect("text layout should exist for text display item")
            .clone();
        let text_align = object.style.text.text_align.map(|a| match a {
            crate::TextAlign::Start => zeno_scene::TextAlign::Start,
            crate::TextAlign::Center => zeno_scene::TextAlign::Center,
            crate::TextAlign::End => zeno_scene::TextAlign::End,
        });
        items.push(DisplayItem {
            item_id: DisplayItemId((index as u32) * 2 + 1),
            spatial_id: SpatialNodeId(index as u32),
            clip_chain_id,
            stacking_context,
            visual_rect,
            payload: DisplayItemPayload::TextRun(DisplayTextRun {
                position: Point::new(
                    object.style.padding.left,
                    object.style.padding.top + text_layout.metrics.ascent,
                ),
                layout: text_layout,
                color: object.style.text.color,
                text_align,
            }),
        });
    }
    if let FrontendObjectKind::Image(image) = &object.kind {
        let resource_key = image.source.resource_key();
        let resource = image_resources
            .resolve(resource_key)
            .expect("image resource should exist for display list item");
        items.push(DisplayItem {
            item_id: DisplayItemId((index as u32) * 2 + 1),
            spatial_id: SpatialNodeId(index as u32),
            clip_chain_id,
            stacking_context,
            visual_rect,
            payload: DisplayItemPayload::Image(DisplayImage::new_rgba8(
                ImageCacheKey(resource_key.0),
                Rect::new(0.0, 0.0, slot.frame.size.width, slot.frame.size.height),
                resource.width,
                resource.height,
                resource.rgba8.clone(),
            )),
        });
    }
    items
}

fn effects_for_object(object: &FrontendObject) -> Vec<Effect> {
    let mut effects = Vec::new();
    if let Some(blur) = object.style.blur {
        effects.push(Effect::Blur { sigma: blur });
    }
    if let Some(shadow) = object.style.drop_shadow {
        effects.push(Effect::DropShadow {
            dx: shadow.dx,
            dy: shadow.dy,
            blur: shadow.blur,
            color: shadow.color,
        });
    }
    effects
}

fn object_creates_stacking_context(style: &crate::Style) -> bool {
    style.layer
        || style.opacity < 1.0
        || style.blend_mode != BlendMode::Normal
        || style.blur.is_some()
        || style.drop_shadow.is_some()
}
