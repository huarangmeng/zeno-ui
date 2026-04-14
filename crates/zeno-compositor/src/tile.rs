use std::collections::{BTreeMap, BTreeSet};

use zeno_core::{Rect, Size};

use crate::composite::{
    CompositeLayerPass, CompositePass, CompositeTileRef, CompositorBlendMode, CompositorLayerId,
    CompositorLayerTree, CompositorSubmission,
};
use crate::damage::DamageRegion;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileId {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileContentHandle(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileContentState {
    Allocated,
    Rasterized,
    Reused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TileResourceKind {
    OffscreenSurface,
    Texture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TileResourceDescriptor {
    pub kind: TileResourceKind,
    pub width: u32,
    pub height: u32,
}

impl TileResourceDescriptor {
    #[must_use]
    pub fn estimated_byte_size(self) -> usize {
        let bytes_per_pixel = match self.kind {
            TileResourceKind::OffscreenSurface | TileResourceKind::Texture => 4usize,
        };
        self.width as usize * self.height as usize * bytes_per_pixel
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TileResourcePoolDelta {
    pub allocated: Vec<(TileContentHandle, TileResourceDescriptor)>,
    pub released: Vec<TileContentHandle>,
    pub reused: Vec<TileContentHandle>,
    pub evicted: Vec<TileContentHandle>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TileResourceEvictionStats {
    pub budget_eviction_count: usize,
    pub age_eviction_count: usize,
    pub descriptor_limit_eviction_count: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TileResourcePool {
    active: BTreeMap<TileContentHandle, TileResourceDescriptor>,
}

impl TileResourcePool {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn synchronize(&mut self, cache: &mut TileCache) -> TileResourcePoolDelta {
        let released = cache.take_released_content_handles();
        let reused = cache.take_reused_content_handles();
        let evicted = cache.take_evicted_content_handles();
        for handle in &released {
            self.active.remove(handle);
        }
        let mut allocated = Vec::new();
        for (_tile_id, slot) in cache.content_slots() {
            match self.active.get(&slot.handle) {
                Some(existing) if existing == &slot.resource => {}
                _ => {
                    self.active.insert(slot.handle, slot.resource);
                    allocated.push((slot.handle, slot.resource));
                }
            }
        }
        TileResourcePoolDelta {
            allocated,
            released,
            reused,
            evicted,
        }
    }

    #[must_use]
    pub fn resource_count(&self) -> usize {
        self.active.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileContentSlot {
    pub handle: TileContentHandle,
    pub generation: u64,
    pub state: TileContentState,
    pub resource: TileResourceDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReusableTileHandle {
    handle: TileContentHandle,
    last_used_generation: u64,
    access_serial: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileGrid {
    viewport: Size,
    tile_width: f32,
    tile_height: f32,
}

impl TileGrid {
    pub const DEFAULT_TILE_WIDTH: f32 = 256.0;
    pub const DEFAULT_TILE_HEIGHT: f32 = 256.0;
    const TILE_EDGE_EPSILON: f32 = 0.0001;

    #[must_use]
    pub fn new(viewport: Size, tile_width: f32, tile_height: f32) -> Self {
        Self {
            viewport,
            tile_width: tile_width.max(1.0),
            tile_height: tile_height.max(1.0),
        }
    }

    #[must_use]
    pub fn for_viewport(viewport: Size) -> Self {
        Self::new(viewport, Self::DEFAULT_TILE_WIDTH, Self::DEFAULT_TILE_HEIGHT)
    }

    #[must_use]
    pub fn tile_count_x(self) -> u32 {
        (self.viewport.width.max(0.0) / self.tile_width).ceil() as u32
    }

    #[must_use]
    pub fn tile_count_y(self) -> u32 {
        (self.viewport.height.max(0.0) / self.tile_height).ceil() as u32
    }

    #[must_use]
    pub fn tile_count(self) -> usize {
        self.tile_count_x() as usize * self.tile_count_y() as usize
    }

    #[must_use]
    pub fn tile_rect(self, tile_id: TileId) -> Option<Rect> {
        if tile_id.x >= self.tile_count_x() || tile_id.y >= self.tile_count_y() {
            return None;
        }
        let left = tile_id.x as f32 * self.tile_width;
        let top = tile_id.y as f32 * self.tile_height;
        let right = (left + self.tile_width).min(self.viewport.width);
        let bottom = (top + self.tile_height).min(self.viewport.height);
        Some(Rect::new(left, top, right - left, bottom - top))
    }

    #[must_use]
    pub fn tiles_for_damage(self, damage: &DamageRegion) -> Vec<TileId> {
        match damage {
            DamageRegion::Empty => Vec::new(),
            DamageRegion::Full => self.all_tiles(),
            DamageRegion::Rects(rects) => {
                let mut tiles = BTreeSet::new();
                for rect in rects {
                    self.collect_tiles_for_rect(*rect, &mut tiles);
                }
                tiles.into_iter().collect()
            }
        }
    }

    #[must_use]
    pub fn dirty_tile_count(self, damage: &DamageRegion) -> usize {
        self.tiles_for_damage(damage).len()
    }

    #[must_use]
    pub fn all_tiles(self) -> Vec<TileId> {
        let mut tiles = Vec::with_capacity(self.tile_count());
        for y in 0..self.tile_count_y() {
            for x in 0..self.tile_count_x() {
                tiles.push(TileId { x, y });
            }
        }
        tiles
    }

    fn collect_tiles_for_rect(self, rect: Rect, tiles: &mut BTreeSet<TileId>) {
        let left = rect.origin.x.max(0.0);
        let top = rect.origin.y.max(0.0);
        let right = rect.right().min(self.viewport.width);
        let bottom = rect.bottom().min(self.viewport.height);
        if right <= left || bottom <= top {
            return;
        }

        let start_x = (left / self.tile_width).floor() as u32;
        let start_y = (top / self.tile_height).floor() as u32;
        let end_x = ((right - Self::TILE_EDGE_EPSILON) / self.tile_width).floor() as u32;
        let end_y = ((bottom - Self::TILE_EDGE_EPSILON) / self.tile_height).floor() as u32;
        let max_x = self.tile_count_x().saturating_sub(1);
        let max_y = self.tile_count_y().saturating_sub(1);

        for y in start_y.min(max_y)..=end_y.min(max_y) {
            for x in start_x.min(max_x)..=end_x.min(max_x) {
                tiles.insert(TileId { x, y });
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TileCacheStats {
    pub total_tile_count: usize,
    pub cached_tile_count: usize,
    pub reraster_tile_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterTile {
    pub tile_id: TileId,
    pub rect: Rect,
    pub content_handle: TileContentHandle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RasterBatch {
    pub tiles: Vec<RasterTile>,
    pub full_raster: bool,
}

impl RasterBatch {
    #[must_use]
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    #[must_use]
    pub fn bounds(&self) -> Option<Rect> {
        self.tiles
            .iter()
            .map(|tile| tile.rect)
            .reduce(|current, rect| current.union(&rect))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TilePlan {
    pub dirty_tiles: Vec<TileId>,
    pub reused_tiles: Vec<TileId>,
    pub stats: TileCacheStats,
}

impl TilePlan {
    #[must_use]
    pub fn dirty_tile_count(&self) -> usize {
        self.stats.reraster_tile_count
    }

    #[must_use]
    pub fn cached_tile_count(&self) -> usize {
        self.stats.cached_tile_count
    }

    #[must_use]
    pub fn total_tile_count(&self) -> usize {
        self.stats.total_tile_count
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TileCache {
    cached_grid: Option<TileGrid>,
    cached_tiles: BTreeSet<TileId>,
    content_slots: BTreeMap<TileId, TileContentSlot>,
    reusable_handles: BTreeMap<TileResourceDescriptor, Vec<ReusableTileHandle>>,
    released_handles: Vec<TileContentHandle>,
    reused_handles: Vec<TileContentHandle>,
    evicted_handles: Vec<TileContentHandle>,
    reusable_byte_count: usize,
    access_serial: u64,
    eviction_stats: TileResourceEvictionStats,
    next_content_handle: u64,
    content_generation: u64,
}

impl TileCache {
    const MAX_REUSABLE_PER_DESCRIPTOR: usize = 8;
    const MAX_REUSABLE_GENERATION_AGE: u64 = 3;
    const MAX_REUSABLE_BYTE_COUNT: usize = 16 * 1024 * 1024;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate_all(&mut self) {
        self.cached_tiles.clear();
        let mut content_slots = BTreeMap::new();
        std::mem::swap(&mut content_slots, &mut self.content_slots);
        for (_, slot) in content_slots {
            self.recycle_slot(slot);
        }
        self.evict_stale_handles(self.content_generation);
        self.cached_grid = None;
    }

    #[must_use]
    pub fn plan_frame(&mut self, grid: TileGrid, damage: &DamageRegion) -> TilePlan {
        let all_tiles = grid.all_tiles();
        let full_rebuild = damage.is_full()
            || self.cached_grid != Some(grid)
            || self.cached_tiles.len() != all_tiles.len();
        if full_rebuild {
            let mut content_slots = BTreeMap::new();
            std::mem::swap(&mut content_slots, &mut self.content_slots);
            for (_, slot) in content_slots {
                self.recycle_slot(slot);
            }
        }
        let dirty_tiles = if full_rebuild {
            all_tiles.clone()
        } else {
            grid.tiles_for_damage(damage)
        };
        let dirty_tile_set: BTreeSet<_> = dirty_tiles.iter().copied().collect();
        let reused_tiles = if full_rebuild {
            Vec::new()
        } else {
            all_tiles
                .iter()
                .copied()
                .filter(|tile_id| self.cached_tiles.contains(tile_id) && !dirty_tile_set.contains(tile_id))
                .collect()
        };
        let plan = TilePlan {
            dirty_tiles,
            reused_tiles,
            stats: TileCacheStats {
                total_tile_count: all_tiles.len(),
                cached_tile_count: all_tiles.len().saturating_sub(dirty_tile_set.len()),
                reraster_tile_count: dirty_tile_set.len(),
            },
        };
        self.cached_grid = Some(grid);
        self.cached_tiles = all_tiles.into_iter().collect();
        plan
    }

    #[must_use]
    pub fn build_submission(
        &mut self,
        grid: TileGrid,
        damage: &DamageRegion,
    ) -> CompositorSubmission {
        self.content_generation += 1;
        self.reused_handles.clear();
        self.evicted_handles.clear();
        self.eviction_stats = TileResourceEvictionStats::default();
        let tile_plan = self.plan_frame(grid, damage);
        let dirty_tile_set: BTreeSet<_> = tile_plan.dirty_tiles.iter().copied().collect();
        for tile_id in &tile_plan.dirty_tiles {
            if let Some(old_slot) = self.content_slots.remove(tile_id) {
                self.recycle_slot(old_slot);
            }
            let resource = self.resource_descriptor_for_tile(grid, *tile_id);
            let handle = self.acquire_handle(resource);
            self.content_slots.insert(
                *tile_id,
                TileContentSlot {
                    handle,
                    generation: self.content_generation,
                    state: TileContentState::Rasterized,
                    resource,
                },
            );
        }
        for tile_id in grid.all_tiles() {
            if !dirty_tile_set.contains(&tile_id) {
                let generation = self.content_generation;
                if let Some(slot) = self.content_slots.get_mut(&tile_id) {
                    slot.generation = generation;
                    slot.state = TileContentState::Reused;
                } else {
                    let resource = self.resource_descriptor_for_tile(grid, tile_id);
                    let handle = self.acquire_handle(resource);
                    self.content_slots.insert(
                        tile_id,
                        TileContentSlot {
                            handle,
                            generation,
                            state: TileContentState::Allocated,
                            resource,
                        },
                    );
                }
            }
        }
        self.evict_stale_handles(self.content_generation);
        let raster_batch = RasterBatch {
            tiles: tile_plan
                .dirty_tiles
                .iter()
                .copied()
                .filter_map(|tile_id| {
                    grid.tile_rect(tile_id).and_then(|rect| {
                        self.content_slots
                            .get(&tile_id)
                            .map(|slot| RasterTile {
                                tile_id,
                                rect,
                                content_handle: slot.handle,
                            })
                    })
                })
                .collect(),
            full_raster: damage.is_full() || tile_plan.dirty_tile_count() == tile_plan.total_tile_count(),
        };
        let composite_pass = CompositePass {
            steps: vec![CompositeLayerPass {
                layer_id: CompositorLayerId(0),
                tiles: (0..grid.tile_count_y())
                    .flat_map(|y| (0..grid.tile_count_x()).map(move |x| TileId { x, y }))
                    .filter_map(|tile_id| {
                        self.content_slots
                            .get(&tile_id)
                            .map(|slot| CompositeTileRef {
                                tile_id,
                                content_handle: slot.handle,
                            })
                    })
                    .collect(),
                needs_offscreen: false,
                opacity: 1.0,
                blend_mode: CompositorBlendMode::Normal,
                effects: Vec::new(),
                bounds: Rect::new(0.0, 0.0, grid.viewport.width, grid.viewport.height),
                effect_bounds: Rect::new(0.0, 0.0, grid.viewport.width, grid.viewport.height),
                effect_padding: 0.0,
            }],
            full_present: damage.is_full(),
        };
        CompositorSubmission {
            tile_plan,
            raster_batch,
            composite_pass,
            layer_tree: CompositorLayerTree::default(),
        }
    }

    #[must_use]
    pub fn content_handle(&self, tile_id: TileId) -> Option<TileContentHandle> {
        self.content_slots.get(&tile_id).map(|slot| slot.handle)
    }

    #[must_use]
    pub fn content_handle_count(&self) -> usize {
        self.content_slots.len()
    }

    #[must_use]
    pub fn content_slot(&self, tile_id: TileId) -> Option<TileContentSlot> {
        self.content_slots.get(&tile_id).copied()
    }

    #[must_use]
    pub fn content_slots(&self) -> Vec<(TileId, TileContentSlot)> {
        self.content_slots
            .iter()
            .map(|(tile_id, slot)| (*tile_id, *slot))
            .collect()
    }

    #[must_use]
    pub fn rasterized_slot_count(&self) -> usize {
        self.content_slots
            .values()
            .filter(|slot| slot.state == TileContentState::Rasterized)
            .count()
    }

    #[must_use]
    pub fn reusable_handle_count(&self) -> usize {
        self.reusable_handles.values().map(Vec::len).sum()
    }

    #[must_use]
    pub fn reusable_byte_count(&self) -> usize {
        self.reusable_byte_count
    }

    #[must_use]
    pub const fn reuse_budget_byte_count(&self) -> usize {
        Self::MAX_REUSABLE_BYTE_COUNT
    }

    pub fn take_released_content_handles(&mut self) -> Vec<TileContentHandle> {
        let mut released_handles = Vec::new();
        std::mem::swap(&mut released_handles, &mut self.released_handles);
        released_handles
    }

    pub fn take_reused_content_handles(&mut self) -> Vec<TileContentHandle> {
        let mut reused_handles = Vec::new();
        std::mem::swap(&mut reused_handles, &mut self.reused_handles);
        reused_handles
    }

    pub fn take_evicted_content_handles(&mut self) -> Vec<TileContentHandle> {
        let mut evicted_handles = Vec::new();
        std::mem::swap(&mut evicted_handles, &mut self.evicted_handles);
        evicted_handles
    }

    #[must_use]
    pub const fn eviction_stats(&self) -> TileResourceEvictionStats {
        self.eviction_stats
    }

    fn allocate_handle(&mut self) -> TileContentHandle {
        let handle = TileContentHandle(self.next_content_handle.max(1));
        self.next_content_handle = handle.0 + 1;
        handle
    }

    fn acquire_handle(&mut self, resource: TileResourceDescriptor) -> TileContentHandle {
        if let Some(handles) = self.reusable_handles.get_mut(&resource)
            && let Some(reusable) = handles.pop()
        {
            self.reusable_byte_count = self
                .reusable_byte_count
                .saturating_sub(resource.estimated_byte_size());
            if handles.is_empty() {
                self.reusable_handles.remove(&resource);
            }
            self.reused_handles.push(reusable.handle);
            return reusable.handle;
        }
        self.allocate_handle()
    }

    fn recycle_slot(&mut self, slot: TileContentSlot) {
        let resource_bytes = slot.resource.estimated_byte_size();
        let access_serial = self.next_access_serial();
        let handles = self.reusable_handles.entry(slot.resource).or_default();
        handles.push(ReusableTileHandle {
            handle: slot.handle,
            last_used_generation: slot.generation,
            access_serial,
        });
        self.reusable_byte_count += resource_bytes;
        if handles.len() > Self::MAX_REUSABLE_PER_DESCRIPTOR {
            if let Some(index) = Self::oldest_handle_index(handles, slot.resource) {
                let released = handles.remove(index);
                self.released_handles.push(released.handle);
                self.evicted_handles.push(released.handle);
                self.eviction_stats.descriptor_limit_eviction_count += 1;
                self.reusable_byte_count = self.reusable_byte_count.saturating_sub(resource_bytes);
            }
        }
    }

    fn evict_stale_handles(&mut self, generation: u64) {
        let mut descriptors_to_remove = Vec::new();
        for (descriptor, handles) in &mut self.reusable_handles {
            let mut retained = Vec::with_capacity(handles.len());
            for reusable in handles.drain(..) {
                if generation.saturating_sub(reusable.last_used_generation)
                    > Self::MAX_REUSABLE_GENERATION_AGE
                {
                    self.released_handles.push(reusable.handle);
                    self.evicted_handles.push(reusable.handle);
                    self.eviction_stats.age_eviction_count += 1;
                    self.reusable_byte_count = self
                        .reusable_byte_count
                        .saturating_sub(descriptor.estimated_byte_size());
                } else {
                    retained.push(reusable);
                }
            }
            *handles = retained;
            if handles.is_empty() {
                descriptors_to_remove.push(*descriptor);
            }
        }
        for descriptor in descriptors_to_remove {
            self.reusable_handles.remove(&descriptor);
        }
        self.evict_over_budget();
    }

    fn evict_over_budget(&mut self) {
        while self.reusable_byte_count > Self::MAX_REUSABLE_BYTE_COUNT {
            let Some((descriptor, index, reusable)) = self.select_budget_eviction_candidate() else {
                break;
            };
            if let Some(handles) = self.reusable_handles.get_mut(&descriptor) {
                handles.remove(index);
                if handles.is_empty() {
                    self.reusable_handles.remove(&descriptor);
                }
            }
            self.released_handles.push(reusable.handle);
            self.evicted_handles.push(reusable.handle);
            self.eviction_stats.budget_eviction_count += 1;
            self.reusable_byte_count = self
                .reusable_byte_count
                .saturating_sub(descriptor.estimated_byte_size());
        }
    }

    fn select_budget_eviction_candidate(
        &self,
    ) -> Option<(TileResourceDescriptor, usize, ReusableTileHandle)> {
        let mut candidate: Option<(TileResourceDescriptor, usize, ReusableTileHandle)> = None;
        for (descriptor, handles) in &self.reusable_handles {
            for (index, reusable) in handles.iter().copied().enumerate() {
                let replace = match candidate {
                    Some((current_descriptor, _, current)) => {
                        reusable.access_serial < current.access_serial
                            || (reusable.access_serial == current.access_serial
                                && descriptor.estimated_byte_size()
                                    > current_descriptor.estimated_byte_size())
                    }
                    None => true,
                };
                if replace {
                    candidate = Some((*descriptor, index, reusable));
                }
            }
        }
        candidate
    }

    fn oldest_handle_index(
        handles: &[ReusableTileHandle],
        _descriptor: TileResourceDescriptor,
    ) -> Option<usize> {
        let mut candidate: Option<(usize, ReusableTileHandle)> = None;
        for (index, reusable) in handles.iter().copied().enumerate() {
            let replace = match candidate {
                Some((_, current)) => {
                    reusable.access_serial < current.access_serial
                        || (reusable.access_serial == current.access_serial
                            && reusable.last_used_generation <= current.last_used_generation)
                }
                None => true,
            };
            if replace {
                candidate = Some((index, reusable));
            }
        }
        candidate.map(|(index, _)| index)
    }

    fn next_access_serial(&mut self) -> u64 {
        self.access_serial += 1;
        self.access_serial
    }

    fn resource_descriptor_for_tile(
        &self,
        grid: TileGrid,
        tile_id: TileId,
    ) -> TileResourceDescriptor {
        let rect = grid.tile_rect(tile_id).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        TileResourceDescriptor {
            kind: TileResourceKind::OffscreenSurface,
            width: rect.size.width.max(0.0).ceil() as u32,
            height: rect.size.height.max(0.0).ceil() as u32,
        }
    }
}
