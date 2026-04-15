use zeno_core::{Rect, Size};

use crate::composite::{
    CompositeLayerPass, CompositePass, CompositeTileRef, CompositorBlendMode, CompositorEffect,
    CompositorLayer, CompositorLayerId, CompositorLayerTree, CompositorScopeEntry,
    CompositorSubmission,
};
use crate::damage::DamageRegion;
use crate::tile::{TileCache, TileGrid};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompositorPlanningItem {
    pub item_index: usize,
    pub paint_order: usize,
    pub stacking_context_index: Option<usize>,
    pub visual_rect: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositorPlanningContext {
    pub parent_context_index: Option<usize>,
    pub paint_order: usize,
    pub opacity: f32,
    pub blend_mode: CompositorBlendMode,
    pub effects: Vec<CompositorEffect>,
    pub needs_offscreen: bool,
}

pub trait CompositorPlanningSource {
    fn viewport(&self) -> Size;

    fn item_count_hint(&self) -> usize;

    fn stacking_context_count_hint(&self) -> usize;

    fn for_each_item(&self, visitor: impl FnMut(CompositorPlanningItem));

    fn for_each_stacking_context(&self, visitor: impl FnMut(CompositorPlanningContext));
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CompositorPlanner;

impl CompositorPlanner {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn plan<S: CompositorPlanningSource>(
        &self,
        source: &S,
        tile_cache: &mut TileCache,
        damage: &DamageRegion,
    ) -> CompositorSubmission {
        let grid = TileGrid::for_viewport(source.viewport());
        let tile_state = tile_cache.build_tile_state(grid, damage);
        let layer_tree = self.build_layer_tree(source, grid);
        let composite_pass = self.build_composite_pass(&layer_tree, damage.is_full(), tile_cache);
        CompositorSubmission {
            tile_plan: tile_state.tile_plan,
            raster_batch: tile_state.raster_batch,
            composite_pass,
            layer_tree,
        }
    }

    fn build_layer_tree<S: CompositorPlanningSource>(
        &self,
        source: &S,
        grid: TileGrid,
    ) -> CompositorLayerTree {
        let root_layer_id = CompositorLayerId(0);
        let mut items = Vec::with_capacity(source.item_count_hint());
        source.for_each_item(|item| items.push(item));
        items.sort_by_key(|item| item.paint_order);

        let mut root_rects = Vec::with_capacity(items.len());
        let mut context_rects = vec![Vec::new(); source.stacking_context_count_hint()];
        let mut root_item_count = 0usize;
        let mut context_item_counts = vec![0usize; source.stacking_context_count_hint()];

        for item in &items {
            if let Some(index) = item.stacking_context_index {
                if index >= context_rects.len() {
                    context_rects.resize(index + 1, Vec::new());
                    context_item_counts.resize(index + 1, 0);
                }
                context_rects[index].push(item.visual_rect);
                context_item_counts[index] += 1;
            } else {
                root_rects.push(item.visual_rect);
                root_item_count += 1;
            }
        }

        let mut contexts = Vec::with_capacity(source.stacking_context_count_hint());
        source.for_each_stacking_context(|context| contexts.push(context));

        let mut layers = Vec::with_capacity(contexts.len() + 1);
        let root_tiles = layer_tiles_for_items(root_rects.iter().copied(), grid);
        let root_bounds =
            rect_bounds(root_rects.iter().copied()).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        layers.push(CompositorLayer {
            layer_id: root_layer_id,
            parent: None,
            child_layers: Vec::new(),
            descendant_layers: Vec::new(),
            scope_entries: Vec::new(),
            stacking_context_index: None,
            paint_order: 0,
            opacity: 1.0,
            blend_mode: CompositorBlendMode::Normal,
            effects: Vec::new(),
            needs_offscreen: false,
            bounds: root_bounds,
            subtree_bounds: root_bounds,
            effect_bounds: root_bounds,
            effect_padding: 0.0,
            item_count: root_item_count,
            tile_ids: root_tiles,
        });

        for (index, context) in contexts.iter().cloned().enumerate() {
            let rects = context_rects.get(index).cloned().unwrap_or_default();
            let bounds =
                rect_bounds(rects.iter().copied()).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            let effect_padding = effect_padding(&context.effects);
            let effect_bounds = effect_bounds(bounds, &context.effects);
            let parent = context
                .parent_context_index
                .map(|parent_index| CompositorLayerId((parent_index + 1) as u32))
                .or(Some(root_layer_id));
            layers.push(CompositorLayer {
                layer_id: CompositorLayerId((index + 1) as u32),
                parent,
                child_layers: Vec::new(),
                descendant_layers: Vec::new(),
                scope_entries: Vec::new(),
                stacking_context_index: Some(index),
                paint_order: context.paint_order,
                opacity: context.opacity,
                blend_mode: context.blend_mode,
                effects: context.effects,
                needs_offscreen: context.needs_offscreen,
                bounds,
                subtree_bounds: bounds,
                effect_bounds,
                effect_padding,
                item_count: context_item_counts.get(index).copied().unwrap_or(0),
                tile_ids: layer_tiles_for_items(rects.into_iter(), grid),
            });
        }

        let index_by_id = layers
            .iter()
            .enumerate()
            .map(|(index, layer)| (layer.layer_id, index))
            .collect::<std::collections::HashMap<_, _>>();
        for layer_index in 1..layers.len() {
            let layer_id = layers[layer_index].layer_id;
            let Some(parent_id) = layers[layer_index].parent else {
                continue;
            };
            let Some(parent_index) = index_by_id.get(&parent_id).copied() else {
                continue;
            };
            layers[parent_index].child_layers.push(layer_id);
        }

        finalize_layer_dependencies(&mut layers, &index_by_id);

        for layer_index in 0..layers.len() {
            let scope_context_index = layers[layer_index].stacking_context_index;
            layers[layer_index].scope_entries =
                build_scope_entries_for_layer(scope_context_index, &items, &contexts);
        }

        CompositorLayerTree { layers }
    }

    fn build_composite_pass(
        &self,
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
                    parent: layer.parent,
                    descendant_layers: layer.descendant_layers.clone(),
                    tiles: layer
                        .tile_ids
                        .iter()
                        .copied()
                        .filter_map(|tile_id| {
                            tile_cache.content_handle(tile_id).map(|content_handle| {
                                CompositeTileRef {
                                    tile_id,
                                    content_handle,
                                }
                            })
                        })
                        .collect(),
                    paint_order: layer.paint_order,
                    needs_offscreen: layer.needs_offscreen,
                    opacity: layer.opacity,
                    blend_mode: layer.blend_mode,
                    effects: layer.effects.clone(),
                    bounds: layer.bounds,
                    subtree_bounds: layer.subtree_bounds,
                    effect_bounds: layer.effect_bounds,
                    effect_padding: layer.effect_padding,
                })
                .collect(),
            full_present,
        }
    }
}

fn finalize_layer_dependencies(
    layers: &mut [CompositorLayer],
    index_by_id: &std::collections::HashMap<CompositorLayerId, usize>,
) {
    for layer_index in (0..layers.len()).rev() {
        let child_ids = layers[layer_index].child_layers.clone();
        let mut descendant_layers = Vec::new();
        let mut subtree_bounds = layers[layer_index].bounds;
        for child_id in child_ids {
            let Some(child_index) = index_by_id.get(&child_id).copied() else {
                continue;
            };
            let child = &layers[child_index];
            descendant_layers.push(child_id);
            descendant_layers.extend(child.descendant_layers.iter().copied());
            subtree_bounds = union_rect(subtree_bounds, child.effect_bounds);
        }
        layers[layer_index].descendant_layers = descendant_layers;
        layers[layer_index].subtree_bounds = subtree_bounds;
        layers[layer_index].effect_bounds = effect_bounds(
            layers[layer_index].subtree_bounds,
            &layers[layer_index].effects,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeEntryKind {
    Skip,
    DirectItem,
    ChildContext(usize),
}

fn build_scope_entries_for_layer(
    scope_context_index: Option<usize>,
    items: &[CompositorPlanningItem],
    contexts: &[CompositorPlanningContext],
) -> Vec<CompositorScopeEntry> {
    let mut rendered_children = std::collections::HashSet::new();
    let mut entries = Vec::new();
    for item in items {
        match scope_entry_for_item(scope_context_index, item.stacking_context_index, contexts) {
            ScopeEntryKind::Skip => {}
            ScopeEntryKind::DirectItem => {
                entries.push(CompositorScopeEntry::DirectItem(item.item_index));
            }
            ScopeEntryKind::ChildContext(child_context_index) => {
                if rendered_children.insert(child_context_index) {
                    entries.push(CompositorScopeEntry::ChildLayer(CompositorLayerId(
                        (child_context_index + 1) as u32,
                    )));
                }
            }
        }
    }
    entries
}

fn scope_entry_for_item(
    scope_context_index: Option<usize>,
    item_context_index: Option<usize>,
    contexts: &[CompositorPlanningContext],
) -> ScopeEntryKind {
    match (scope_context_index, item_context_index) {
        (None, None) => ScopeEntryKind::DirectItem,
        (Some(parent), Some(current)) if current == parent => ScopeEntryKind::DirectItem,
        (scope_parent, Some(current)) => immediate_child_context(scope_parent, current, contexts)
            .map(ScopeEntryKind::ChildContext)
            .unwrap_or(ScopeEntryKind::Skip),
        _ => ScopeEntryKind::Skip,
    }
}

fn immediate_child_context(
    parent_context_index: Option<usize>,
    mut current: usize,
    contexts: &[CompositorPlanningContext],
) -> Option<usize> {
    let mut path = vec![current];
    while let Some(parent) = contexts
        .get(current)
        .and_then(|context| context.parent_context_index)
    {
        path.push(parent);
        current = parent;
    }
    path.reverse();
    match parent_context_index {
        None => path.first().copied(),
        Some(parent) => path
            .iter()
            .position(|&index| index == parent)
            .and_then(|index| path.get(index + 1).copied()),
    }
}

fn layer_tiles_for_items(
    rects: impl IntoIterator<Item = Rect>,
    grid: TileGrid,
) -> Vec<crate::TileId> {
    let damage = DamageRegion::from_rects(rects);
    grid.tiles_for_damage(&damage)
}

fn rect_bounds(rects: impl IntoIterator<Item = Rect>) -> Option<Rect> {
    rects
        .into_iter()
        .reduce(|current, rect| current.union(&rect))
}

fn union_rect(current: Rect, next: Rect) -> Rect {
    if is_zero_rect(current) {
        return next;
    }
    if is_zero_rect(next) {
        return current;
    }
    current.union(&next)
}

fn is_zero_rect(rect: Rect) -> bool {
    rect.size.width <= 0.0 || rect.size.height <= 0.0
}

fn effect_padding(effects: &[CompositorEffect]) -> f32 {
    effects.iter().fold(0.0, |padding, effect| match effect {
        CompositorEffect::Blur { sigma } => padding.max(sigma * 3.0),
        CompositorEffect::DropShadow { dx, dy, blur, .. } => {
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

fn effect_bounds(bounds: Rect, effects: &[CompositorEffect]) -> Rect {
    effects.iter().fold(bounds, |current, effect| match effect {
        CompositorEffect::Blur { sigma } => current.union(&inflate_rect(bounds, sigma * 3.0)),
        CompositorEffect::DropShadow { dx, dy, blur, .. } => {
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
