use std::{ops::Range, sync::Arc};

use zeno_compositor::{
    CompositorBlendMode, CompositorEffect, CompositorPlanningContext, CompositorPlanningItem,
    CompositorPlanningSource,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayTextRun {
    pub position: Point,
    pub layout: TextLayout,
    pub color: Color,
    pub text_align: Option<TextAlign>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayImage {
    pub cache_key: ImageCacheKey,
    pub dest_rect: Rect,
    pub width: u32,
    pub height: u32,
    pub rgba8: Arc<[u8]>,
}

impl DisplayImage {
    #[must_use]
    pub fn new_rgba8(
        cache_key: ImageCacheKey,
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
            cache_key,
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
    pub parent: Option<StackingContextId>,
    pub paint_order: usize,
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
}

impl CompositorPlanningSource for DisplayList {
    fn viewport(&self) -> Size {
        self.viewport
    }

    fn item_count_hint(&self) -> usize {
        self.items.len()
    }

    fn stacking_context_count_hint(&self) -> usize {
        self.stacking_contexts.len()
    }

    fn for_each_item(&self, mut visitor: impl FnMut(CompositorPlanningItem)) {
        let context_indices = self
            .stacking_contexts
            .iter()
            .enumerate()
            .map(|(index, context)| (context.id, index))
            .collect::<std::collections::HashMap<_, _>>();
        for (item_index, item) in self.items.iter().enumerate() {
            visitor(CompositorPlanningItem {
                item_index,
                paint_order: item_index,
                stacking_context_index: item
                    .stacking_context
                    .and_then(|context_id| context_indices.get(&context_id).copied()),
                visual_rect: item.visual_rect,
            });
        }
    }

    fn for_each_stacking_context(&self, mut visitor: impl FnMut(CompositorPlanningContext)) {
        let context_indices = self
            .stacking_contexts
            .iter()
            .enumerate()
            .map(|(index, context)| (context.id, index))
            .collect::<std::collections::HashMap<_, _>>();
        for context in &self.stacking_contexts {
            visitor(CompositorPlanningContext {
                parent_context_index: context
                    .parent
                    .and_then(|parent_id| context_indices.get(&parent_id).copied()),
                paint_order: context.paint_order,
                opacity: context.opacity,
                blend_mode: compositor_blend_mode(context.blend_mode),
                effects: context
                    .effects
                    .iter()
                    .cloned()
                    .map(compositor_effect)
                    .collect(),
                needs_offscreen: context.needs_offscreen,
            });
        }
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
        Effect::DropShadow {
            dx,
            dy,
            blur,
            color,
        } => CompositorEffect::DropShadow {
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
    use crate::{
        CompositorBlendMode, CompositorEffect, CompositorLayerId, CompositorPlanner, DamageRegion,
        TileCache,
    };
    use zeno_core::{Color, Rect, Size};

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
                parent: None,
                paint_order: 1,
                spatial_id: SpatialNodeId(0),
                opacity: 0.5,
                blend_mode: BlendMode::Multiply,
                effects: vec![Effect::Blur { sigma: 4.0 }],
                needs_offscreen: true,
            }],
            generation: 1,
        };

        let submission = CompositorPlanner::new().plan(
            &display_list,
            &mut cache,
            &DamageRegion::from_rects([Rect::new(0.0, 0.0, 40.0, 40.0)]),
        );

        assert_eq!(submission.layer_tree.layer_count(), 2);
        assert_eq!(submission.layer_tree.offscreen_layer_count(), 1);
        assert_eq!(submission.layer_tree.layers[0].item_count, 1);
        assert_eq!(submission.layer_tree.layers[1].item_count, 1);
        assert_eq!(submission.composite_pass.layer_count(), 2);
        assert_eq!(
            submission.composite_pass.steps[1].layer_id,
            CompositorLayerId(1)
        );
        assert!(submission.composite_pass.steps[1].needs_offscreen);
        assert_eq!(
            submission.layer_tree.layers[1].blend_mode,
            CompositorBlendMode::Multiply
        );
        assert_eq!(
            submission.layer_tree.layers[1].effects,
            vec![CompositorEffect::Blur { sigma: 4.0 }]
        );
        assert!(submission.layer_tree.layers[1].effect_padding > 0.0);
        assert!(submission.layer_tree.layers[1].effect_bounds.size.width >= 40.0);
    }
}
