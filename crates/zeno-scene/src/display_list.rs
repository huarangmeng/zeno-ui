use std::{ops::Range, sync::Arc};

use zeno_compositor::{
    CompositeLayerPass, CompositePass, CompositeTileRef, CompositorBlendMode, CompositorEffect,
    CompositorLayer, CompositorLayerId, CompositorLayerTree, CompositorSubmission, DamageRegion,
    TileCache, TileGrid,
};
use zeno_core::{Color, Point, Rect, Size, Transform2D};
use zeno_text::TextLayout;

/// DisplayList is the paint-stage output. It describes what to draw, not how to render.
/// This module intentionally does not provide any compatibility/bridge APIs to legacy scene types.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayList {
    pub viewport: Size,
    pub items: Vec<DisplayItem>,
    pub spatial_tree: SpatialTree,
    pub clip_chains: ClipChainStore,
    pub stacking_contexts: Vec<StackingContext>,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DisplayItemId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpatialNodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClipChainId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StackingContextId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayItem {
    pub item_id: DisplayItemId,
    pub spatial_id: SpatialNodeId,
    pub clip_chain_id: ClipChainId,
    pub stacking_context: Option<StackingContextId>,
    pub visual_rect: Rect,
    pub payload: DisplayItemPayload,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayItemPayload {
    FillRect {
        rect: Rect,
        color: Color,
    },
    FillRoundedRect {
        rect: Rect,
        radius: f32,
        color: Color,
    },
    TextRun(DisplayTextRun),
    Image(DisplayImage),
    Custom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayTextRun {
    pub position: Point,
    pub layout: TextLayout,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayImage {
    pub dest_rect: Rect,
    pub width: u32,
    pub height: u32,
    pub rgba8: Arc<[u8]>,
}

impl DisplayImage {
    #[must_use]
    pub fn new_rgba8(
        dest_rect: Rect,
        width: u32,
        height: u32,
        rgba8: impl Into<Arc<[u8]>>,
    ) -> Self {
        let rgba8 = rgba8.into();
        debug_assert_eq!(
            rgba8.len(),
            (width as usize) * (height as usize) * 4,
            "DisplayImage expects RGBA8 pixel storage"
        );
        Self {
            dest_rect,
            width,
            height,
            rgba8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextCacheKey(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageCacheKey(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub struct SpatialTree {
    pub nodes: Vec<SpatialNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpatialNode {
    pub id: SpatialNodeId,
    pub parent: Option<SpatialNodeId>,
    pub local_transform: Transform2D,
    pub world_transform: Transform2D,
    pub dirty: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClipChainStore {
    pub chains: Vec<ClipChain>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClipChain {
    pub id: ClipChainId,
    pub spatial_id: SpatialNodeId,
    pub clip: ClipRegion,
    pub parent: Option<ClipChainId>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClipRegion {
    Rect(Rect),
    RoundedRect { rect: Rect, radius: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct StackingContext {
    pub id: StackingContextId,
    pub spatial_id: SpatialNodeId,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub effects: Vec<Effect>,
    pub needs_offscreen: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Blur {
        sigma: f32,
    },
    DropShadow {
        dx: f32,
        dy: f32,
        blur: f32,
        color: Color,
    },
}

/// A retained (incrementally updatable) DisplayList data model for paint-stage caching.
#[derive(Debug, Clone, PartialEq)]
pub struct RetainedDisplayList {
    pub items: Vec<DisplayItem>,
    pub spatial_tree: SpatialTree,
    pub clip_chains: ClipChainStore,
    pub stacking_contexts: Vec<StackingContext>,
    pub object_item_ranges: Vec<Option<Range<usize>>>,
    pub free_item_slots: Vec<usize>,
    pub generation: u64,
}

impl DisplayList {
    #[must_use]
    pub fn empty(viewport: Size) -> Self {
        Self {
            viewport,
            items: Vec::new(),
            spatial_tree: SpatialTree { nodes: Vec::new() },
            clip_chains: ClipChainStore { chains: Vec::new() },
            stacking_contexts: Vec::new(),
            generation: 0,
        }
    }

    #[must_use]
    pub fn build_compositor_submission(
        &self,
        tile_cache: &mut TileCache,
        damage: &DamageRegion,
    ) -> CompositorSubmission {
        let grid = TileGrid::for_viewport(self.viewport);
        let mut submission = tile_cache.build_submission(grid, damage);
        submission.layer_tree = build_compositor_layer_tree(self, grid);
        submission.composite_pass =
            build_composite_pass(
                &submission.layer_tree,
                submission.raster_batch.full_raster,
                tile_cache,
            );
        submission
    }
}

impl RetainedDisplayList {
    #[must_use]
    pub fn new(_viewport: Size) -> Self {
        Self {
            items: Vec::new(),
            spatial_tree: SpatialTree { nodes: Vec::new() },
            clip_chains: ClipChainStore { chains: Vec::new() },
            stacking_contexts: Vec::new(),
            object_item_ranges: Vec::new(),
            free_item_slots: Vec::new(),
            generation: 0,
        }
    }

    pub fn ensure_object_capacity(&mut self, object_index: usize) {
        if self.object_item_ranges.len() <= object_index {
            self.object_item_ranges.resize(object_index + 1, None);
        }
    }

    pub fn replace_object_items(&mut self, object_index: usize, new_items: Vec<DisplayItem>) {
        self.ensure_object_capacity(object_index);
        if let Some(old_range) = self.object_item_ranges[object_index].take() {
            for slot in old_range {
                if slot < self.items.len() {
                    self.free_item_slots.push(slot);
                }
            }
        }

        if new_items.is_empty() {
            self.object_item_ranges[object_index] = None;
            self.generation += 1;
            return;
        }

        // This retained model stores a single contiguous item range per object.
        // To keep the invariant simple and predictable, updates append to the tail.
        // Compaction is a separate explicit step (or an internal heuristic) that can rewrite ranges.
        let start = self.items.len();
        self.items.extend(new_items);
        self.object_item_ranges[object_index] = Some(start..self.items.len());
        self.generation += 1;
    }

    pub fn remove_object_items(&mut self, object_index: usize) {
        self.ensure_object_capacity(object_index);
        if let Some(old_range) = self.object_item_ranges[object_index].take() {
            for slot in old_range {
                if slot < self.items.len() {
                    self.free_item_slots.push(slot);
                }
            }
            self.generation += 1;
        }
    }

    pub fn compact_if_needed(&mut self) {
        if self.free_item_slots.is_empty() {
            return;
        }
        let free = self.free_item_slots.len();
        let live = self
            .object_item_ranges
            .iter()
            .flatten()
            .map(|range| range.end.saturating_sub(range.start))
            .sum::<usize>();
        let should_compact = live == 0 || free >= live / 2 || self.free_item_slots.len() > 64;
        if should_compact {
            self.compact_items();
        }
    }

    #[must_use]
    pub fn snapshot(&self, viewport: Size) -> DisplayList {
        let mut items = Vec::new();
        for range in self.object_item_ranges.iter().flatten() {
            items.extend(self.items[range.clone()].iter().cloned());
        }
        DisplayList {
            viewport,
            items,
            spatial_tree: self.spatial_tree.clone(),
            clip_chains: self.clip_chains.clone(),
            stacking_contexts: self.stacking_contexts.clone(),
            generation: self.generation,
        }
    }

    #[must_use]
    pub fn bounds_for_object_indices(
        &self,
        object_indices: impl IntoIterator<Item = usize>,
    ) -> Option<Rect> {
        let mut bounds: Option<Rect> = None;
        for object_index in object_indices {
            let Some(range) = self
                .object_item_ranges
                .get(object_index)
                .and_then(|range| range.clone())
            else {
                continue;
            };
            for item in &self.items[range] {
                bounds = Some(match bounds {
                    Some(current) => current.union(&item.visual_rect),
                    None => item.visual_rect,
                });
            }
        }
        bounds
    }

    fn compact_items(&mut self) {
        let mut rebuilt = Vec::new();
        let mut rebuilt_ranges = vec![None; self.object_item_ranges.len()];
        for (object_index, range) in self.object_item_ranges.iter().enumerate() {
            let Some(range) = range else {
                continue;
            };
            let start = rebuilt.len();
            rebuilt.extend(self.items[range.clone()].iter().cloned());
            rebuilt_ranges[object_index] = Some(start..rebuilt.len());
        }
        self.items = rebuilt;
        self.object_item_ranges = rebuilt_ranges;
        self.free_item_slots.clear();
    }
}

fn build_compositor_layer_tree(display_list: &DisplayList, grid: TileGrid) -> CompositorLayerTree {
    let root_layer_id = CompositorLayerId(0);
    let mut layers = Vec::with_capacity(display_list.stacking_contexts.len() + 1);
    let root_rects = display_list
        .items
        .iter()
        .filter(|item| item.stacking_context.is_none())
        .map(|item| item.visual_rect)
        .collect::<Vec<_>>();
    let root_tiles = layer_tiles_for_items(
        root_rects.iter().copied(),
        grid,
    );
    let root_bounds = rect_bounds(root_rects.iter().copied()).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
    layers.push(CompositorLayer {
        layer_id: root_layer_id,
        parent: None,
        stacking_context_index: None,
        opacity: 1.0,
        blend_mode: CompositorBlendMode::Normal,
        effects: Vec::new(),
        needs_offscreen: false,
        bounds: root_bounds,
        effect_bounds: root_bounds,
        effect_padding: 0.0,
        item_count: display_list
            .items
            .iter()
            .filter(|item| item.stacking_context.is_none())
            .count(),
        tile_ids: root_tiles,
    });
    for (index, context) in display_list.stacking_contexts.iter().enumerate() {
        let context_rects = display_list
            .items
            .iter()
            .filter(|item| item.stacking_context == Some(context.id))
            .map(|item| item.visual_rect)
            .collect::<Vec<_>>();
        let tile_ids = layer_tiles_for_items(
            context_rects.iter().copied(),
            grid,
        );
        let item_count = display_list
            .items
            .iter()
            .filter(|item| item.stacking_context == Some(context.id))
            .count();
        let bounds = rect_bounds(context_rects.iter().copied()).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        let effect_bounds = effect_bounds(bounds, &context.effects);
        layers.push(CompositorLayer {
            layer_id: CompositorLayerId((index + 1) as u32),
            parent: Some(root_layer_id),
            stacking_context_index: Some(index),
            opacity: context.opacity,
            blend_mode: compositor_blend_mode(context.blend_mode),
            effects: context.effects.iter().cloned().map(compositor_effect).collect(),
            needs_offscreen: context.needs_offscreen,
            bounds,
            effect_bounds,
            effect_padding: effect_padding(&context.effects),
            item_count,
            tile_ids,
        });
    }
    CompositorLayerTree { layers }
}

fn layer_tiles_for_items(
    rects: impl IntoIterator<Item = Rect>,
    grid: TileGrid,
) -> Vec<zeno_compositor::TileId> {
    let damage = DamageRegion::from_rects(rects);
    grid.tiles_for_damage(&damage)
}

fn build_composite_pass(
    layer_tree: &CompositorLayerTree,
    full_present: bool,
    tile_cache: &TileCache,
) -> CompositePass {
    CompositePass {
        steps: layer_tree
            .layers
            .iter()
            .map(|layer| CompositeLayerPass {
                layer_id: layer.layer_id,
                tiles: layer
                    .tile_ids
                    .iter()
                    .copied()
                    .filter_map(|tile_id| {
                        tile_cache
                            .content_handle(tile_id)
                            .map(|content_handle| CompositeTileRef {
                                tile_id,
                                content_handle,
                            })
                    })
                    .collect(),
                needs_offscreen: layer.needs_offscreen,
                opacity: layer.opacity,
                blend_mode: layer.blend_mode,
                effects: layer.effects.clone(),
                bounds: layer.bounds,
                effect_bounds: layer.effect_bounds,
                effect_padding: layer.effect_padding,
            })
            .collect(),
        full_present,
    }
}

fn rect_bounds(rects: impl IntoIterator<Item = Rect>) -> Option<Rect> {
    rects.into_iter().reduce(|current, rect| current.union(&rect))
}

fn effect_padding(effects: &[Effect]) -> f32 {
    effects.iter().fold(0.0, |padding, effect| match effect {
        Effect::Blur { sigma } => padding.max(sigma * 3.0),
        Effect::DropShadow { dx, dy, blur, .. } => {
            padding.max(blur * 3.0 + dx.abs().max(dy.abs()))
        }
    })
}

fn inflate_rect(rect: Rect, padding: f32) -> Rect {
    Rect::new(
        rect.origin.x - padding,
        rect.origin.y - padding,
        rect.size.width + padding * 2.0,
        rect.size.height + padding * 2.0,
    )
}

fn effect_bounds(bounds: Rect, effects: &[Effect]) -> Rect {
    effects.iter().fold(bounds, |current, effect| match effect {
        Effect::Blur { sigma } => current.union(&inflate_rect(bounds, sigma * 3.0)),
        Effect::DropShadow { dx, dy, blur, .. } => {
            let shadow = inflate_rect(
                Rect::new(
                    bounds.origin.x + dx,
                    bounds.origin.y + dy,
                    bounds.size.width,
                    bounds.size.height,
                ),
                blur * 3.0,
            );
            current.union(&shadow)
        }
    })
}

fn compositor_blend_mode(mode: BlendMode) -> CompositorBlendMode {
    match mode {
        BlendMode::Normal => CompositorBlendMode::Normal,
        BlendMode::Multiply => CompositorBlendMode::Multiply,
        BlendMode::Screen => CompositorBlendMode::Screen,
    }
}

fn compositor_effect(effect: Effect) -> CompositorEffect {
    match effect {
        Effect::Blur { sigma } => CompositorEffect::Blur { sigma },
        Effect::DropShadow { dx, dy, blur, color } => CompositorEffect::DropShadow {
            dx,
            dy,
            blur,
            color,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BlendMode, ClipChainId, ClipChainStore, DisplayItem, DisplayItemId, DisplayItemPayload,
        DisplayList, Effect, RetainedDisplayList, SpatialNodeId, SpatialTree, StackingContext,
        StackingContextId,
    };
    use zeno_core::{Color, Rect, Size};
    use crate::{
        CompositorBlendMode, CompositorEffect, CompositorLayerId, DamageRegion, TileCache,
    };

    fn rect_item(item_id: u32, width: f32) -> DisplayItem {
        DisplayItem {
            item_id: DisplayItemId(item_id),
            spatial_id: SpatialNodeId(0),
            clip_chain_id: ClipChainId(0),
            stacking_context: None,
            visual_rect: Rect::new(0.0, 0.0, width, 10.0),
            payload: DisplayItemPayload::FillRect {
                rect: Rect::new(0.0, 0.0, width, 10.0),
                color: Color::WHITE,
            },
        }
    }

    #[test]
    fn replace_and_remove_object_item_ranges() {
        let mut list = RetainedDisplayList::new(Size::new(100.0, 50.0));
        list.spatial_tree = SpatialTree { nodes: Vec::new() };
        list.clip_chains = ClipChainStore { chains: Vec::new() };

        list.replace_object_items(0, vec![rect_item(1, 10.0), rect_item(2, 12.0)]);
        list.replace_object_items(1, vec![rect_item(3, 8.0)]);

        assert_eq!(list.object_item_ranges[0], Some(0..2));
        assert_eq!(list.object_item_ranges[1], Some(2..3));

        list.remove_object_items(0);
        assert!(list.object_item_ranges[0].is_none());
        assert_eq!(list.free_item_slots.len(), 2);

        list.compact_if_needed();
        assert_eq!(list.object_item_ranges[1], Some(0..1));
        assert!(list.free_item_slots.is_empty());
    }

    #[test]
    fn snapshot_flattens_live_object_ranges_only() {
        let viewport = Size::new(100.0, 50.0);
        let mut list = RetainedDisplayList::new(viewport);
        list.replace_object_items(0, vec![rect_item(1, 10.0)]);
        list.replace_object_items(1, vec![rect_item(2, 20.0)]);
        list.remove_object_items(0);

        let snapshot = list.snapshot(viewport);
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].item_id, DisplayItemId(2));
    }

    #[test]
    fn display_list_builds_compositor_submission_with_layer_tree() {
        let viewport = Size::new(256.0, 256.0);
        let mut cache = TileCache::new();
        let display_list = DisplayList {
            viewport,
            items: vec![
                DisplayItem {
                    item_id: DisplayItemId(1),
                    spatial_id: SpatialNodeId(0),
                    clip_chain_id: ClipChainId(0),
                    stacking_context: None,
                    visual_rect: Rect::new(0.0, 0.0, 40.0, 40.0),
                    payload: DisplayItemPayload::FillRect {
                        rect: Rect::new(0.0, 0.0, 40.0, 40.0),
                        color: Color::WHITE,
                    },
                },
                DisplayItem {
                    item_id: DisplayItemId(2),
                    spatial_id: SpatialNodeId(0),
                    clip_chain_id: ClipChainId(0),
                    stacking_context: Some(StackingContextId(1)),
                    visual_rect: Rect::new(100.0, 100.0, 40.0, 40.0),
                    payload: DisplayItemPayload::FillRect {
                        rect: Rect::new(100.0, 100.0, 40.0, 40.0),
                        color: Color::WHITE,
                    },
                },
            ],
            spatial_tree: SpatialTree { nodes: Vec::new() },
            clip_chains: ClipChainStore { chains: Vec::new() },
            stacking_contexts: vec![StackingContext {
                id: StackingContextId(1),
                spatial_id: SpatialNodeId(0),
                opacity: 0.5,
                blend_mode: BlendMode::Multiply,
                effects: vec![Effect::Blur { sigma: 4.0 }],
                needs_offscreen: true,
            }],
            generation: 1,
        };

        let submission =
            display_list.build_compositor_submission(&mut cache, &DamageRegion::from_rects([Rect::new(0.0, 0.0, 40.0, 40.0)]));

        assert_eq!(submission.layer_tree.layer_count(), 2);
        assert_eq!(submission.layer_tree.offscreen_layer_count(), 1);
        assert_eq!(submission.layer_tree.layers[0].item_count, 1);
        assert_eq!(submission.layer_tree.layers[1].item_count, 1);
        assert_eq!(submission.composite_pass.layer_count(), 2);
        assert_eq!(submission.composite_pass.steps[1].layer_id, CompositorLayerId(1));
        assert!(submission.composite_pass.steps[1].needs_offscreen);
        assert_eq!(submission.layer_tree.layers[1].blend_mode, CompositorBlendMode::Multiply);
        assert_eq!(
            submission.layer_tree.layers[1].effects,
            vec![CompositorEffect::Blur { sigma: 4.0 }]
        );
        assert!(submission.layer_tree.layers[1].effect_padding > 0.0);
        assert!(submission.layer_tree.layers[1].effect_bounds.size.width >= 40.0);
    }
}
