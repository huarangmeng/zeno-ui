use std::collections::HashMap;

use zeno_core::{Color, Rect, Size};

use crate::{
    DrawCommand, DrawPacketRange, LayerObject, RenderObject, RenderObjectDelta, Scene,
    SceneResourceKey,
};

/// RetainedScene is the P3-1 stepping stone towards DisplayList + Compositor:
/// - A single mutable scene graph instance is the runtime truth.
/// - Patch updates are applied in-place (no Scene snapshot rebuild, no apply_delta()).
/// - Backends should consume the retained scene directly, using the cached traversal data.
#[derive(Debug, Clone)]
pub struct RetainedScene {
    pub size: Size,
    pub clear_color: Option<Color>,

    packets: Vec<DrawCommand>,
    free_packet_ranges: Vec<DrawPacketRange>,
    live_packet_count: usize,

    // Stable storage with tombstones (index stability is important for cached traversal).
    layers: Vec<LayerEntry>,
    objects: Vec<ObjectEntry>,
    live_layers: usize,
    live_objects: usize,

    layer_index_by_id: HashMap<u64, usize>,
    object_index_by_id: HashMap<u64, usize>,
    free_layer_slots: Vec<usize>,
    free_object_slots: Vec<usize>,

    cache: RenderCache,
    generation: u64,
}

#[derive(Debug, Clone)]
struct LayerEntry {
    live: bool,
    layer: LayerObject,
}

#[derive(Debug, Clone)]
struct ObjectEntry {
    live: bool,
    object: RenderObject,
}

#[derive(Debug, Clone, Default)]
pub struct RenderCache {
    // layer_index -> child layer indices
    child_layers: Vec<Vec<usize>>,
    // layer_index -> object indices
    objects_by_layer: Vec<Vec<usize>>,
    // layer_index -> parent layer index
    parent_layers: Vec<Option<usize>>,
    // Cached flattened subtree ops per layer.
    subtree_ops: Vec<Vec<DrawOp>>,
    // Whether a layer subtree cache needs rebuilding.
    subtree_dirty: Vec<bool>,
    // A flattened traversal stream for backends (enter/objects/children/exit).
    draw_ops: Vec<DrawOp>,
    draw_ops_dirty: bool,
    resource_key_count: usize,
    clear_packet: Option<Color>,
    derived_dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawOp {
    EnterLayer(usize),
    DrawObject(usize),
    ExitLayer(usize),
}

pub enum RetainedSceneUpdate<'a> {
    Full {
        scene: &'a mut RetainedScene,
    },
    Delta {
        scene: &'a mut RetainedScene,
        delta: &'a RenderObjectDelta,
        dirty_bounds: Option<Rect>,
    },
}

impl RetainedScene {
    #[must_use]
    pub fn new(size: Size) -> Self {
        let mut scene = Self {
            size,
            clear_color: None,
            packets: Vec::new(),
            free_packet_ranges: Vec::new(),
            live_packet_count: 0,
            layers: Vec::new(),
            objects: Vec::new(),
            live_layers: 0,
            live_objects: 0,
            layer_index_by_id: HashMap::new(),
            object_index_by_id: HashMap::new(),
            free_layer_slots: Vec::new(),
            free_object_slots: Vec::new(),
            cache: RenderCache::default(),
            generation: 0,
        };
        // Always keep a root layer present.
        scene.insert_layer(LayerObject::root(size));
        scene
    }

    #[must_use]
    pub fn from_scene(scene: Scene) -> Self {
        let mut retained = Self::new(scene.size);
        retained.clear_color = scene.clear_color;

        // Replace root layer if provided.
        retained.layers.clear();
        retained.layer_index_by_id.clear();
        retained.free_layer_slots.clear();
        retained.live_layers = 0;
        for layer in scene.layer_graph {
            retained.insert_layer(layer);
        }
        // Packets are already compacted in Scene.
        retained.packets = scene.packets;
        retained.live_packet_count = retained.packets.len();
        retained.free_packet_ranges.clear();

        retained.objects.clear();
        retained.object_index_by_id.clear();
        retained.free_object_slots.clear();
        retained.live_objects = 0;
        for object in scene.objects {
            retained.insert_object_with_range(object);
        }
        retained.cache.draw_ops_dirty = true;
        retained.cache.derived_dirty = true;
        retained.generation += 1;
        retained
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn packet_count(&mut self) -> usize {
        self.refresh_derived_queries_if_needed();
        self.live_packet_count
            + usize::from(self.clear_color.is_some() && self.cache.clear_packet.is_none())
    }

    #[must_use]
    pub fn clear_packet(&mut self) -> Option<Color> {
        self.refresh_derived_queries_if_needed();
        self.cache.clear_packet
    }

    #[must_use]
    pub fn resource_keys(&self) -> Vec<SceneResourceKey> {
        self.objects
            .iter()
            .filter(|e| e.live)
            .flat_map(|e| e.object.resource_keys.iter().copied())
            .collect()
    }

    #[must_use]
    pub fn resource_key_count(&mut self) -> usize {
        self.refresh_derived_queries_if_needed();
        self.cache.resource_key_count
    }

    #[must_use]
    pub fn snapshot_scene(&self) -> Scene {
        let layers = self
            .layers
            .iter()
            .filter(|entry| entry.live)
            .map(|entry| entry.layer.clone())
            .collect();
        let objects = self
            .objects
            .iter()
            .filter(|entry| entry.live)
            .map(|entry| entry.object.clone())
            .collect::<Vec<_>>();
        let (packets, objects) = self.compact_live_objects(objects);
        Scene::from_layers_and_objects_with_packets(
            self.size,
            self.clear_color,
            layers,
            packets,
            objects,
        )
    }

    #[must_use]
    pub fn live_object_count(&self) -> usize {
        self.live_objects
    }

    #[must_use]
    pub fn live_layer_count(&self) -> usize {
        self.live_layers
    }

    #[must_use]
    pub fn layer(&self, layer_index: usize) -> &LayerObject {
        &self.layers[layer_index].layer
    }

    #[must_use]
    pub fn object(&self, object_index: usize) -> &RenderObject {
        &self.objects[object_index].object
    }

    #[must_use]
    pub fn iter_live_layers(&self) -> impl Iterator<Item = &LayerObject> {
        self.layers.iter().filter(|e| e.live).map(|e| &e.layer)
    }

    #[must_use]
    pub fn iter_live_objects(&self) -> impl Iterator<Item = &RenderObject> {
        self.objects.iter().filter(|e| e.live).map(|e| &e.object)
    }

    #[must_use]
    pub fn packets(&self) -> &[DrawCommand] {
        &self.packets
    }

    #[must_use]
    pub fn packets_for_object_index(&self, object_index: usize) -> &[DrawCommand] {
        let object = self.object(object_index);
        self.packets_for_range(object.packets)
    }

    #[must_use]
    pub fn draw_ops(&mut self) -> &[DrawOp] {
        self.rebuild_cache_if_needed();
        &self.cache.draw_ops
    }

    pub fn rebuild_cache_if_needed(&mut self) {
        if !self.cache.draw_ops_dirty {
            return;
        }
        let root_index = self
            .layer_index_by_id
            .get(&Scene::ROOT_LAYER_ID)
            .copied()
            .unwrap_or(0);
        self.ensure_cache_capacity(root_index);
        self.rebuild_subtree_ops_if_needed(root_index);
        self.cache.draw_ops.clear();
        self.cache.draw_ops.extend_from_slice(&self.cache.subtree_ops[root_index]);
        self.cache.draw_ops_dirty = false;
    }

    fn ensure_cache_capacity(&mut self, layer_index: usize) {
        if self.cache.child_layers.len() <= layer_index {
            self.cache.child_layers.resize(layer_index + 1, Vec::new());
        }
        if self.cache.objects_by_layer.len() <= layer_index {
            self.cache.objects_by_layer.resize(layer_index + 1, Vec::new());
        }
        if self.cache.parent_layers.len() <= layer_index {
            self.cache.parent_layers.resize(layer_index + 1, None);
        }
        if self.cache.subtree_ops.len() <= layer_index {
            self.cache.subtree_ops.resize(layer_index + 1, Vec::new());
        }
        if self.cache.subtree_dirty.len() <= layer_index {
            self.cache.subtree_dirty.resize(layer_index + 1, true);
        }
    }

    fn rebuild_adjacency_from_storage(&mut self) {
        self.cache.child_layers = vec![Vec::new(); self.layers.len()];
        self.cache.objects_by_layer = vec![Vec::new(); self.layers.len()];
        self.cache.parent_layers = vec![None; self.layers.len()];
        self.cache.subtree_ops = vec![Vec::new(); self.layers.len()];
        self.cache.subtree_dirty = vec![true; self.layers.len()];

        for idx in 0..self.layers.len() {
            if !self.layers[idx].live {
                continue;
            }
            self.insert_layer_into_adjacency(idx);
        }
        for idx in 0..self.objects.len() {
            if !self.objects[idx].live {
                continue;
            }
            self.insert_object_into_adjacency(idx);
        }
        self.cache.draw_ops_dirty = true;
    }

    fn insert_layer_into_adjacency(&mut self, layer_index: usize) {
        if layer_index >= self.layers.len() || !self.layers[layer_index].live {
            return;
        }
        self.ensure_cache_capacity(layer_index);
        let Some(parent_id) = self.layers[layer_index].layer.parent_layer_id else {
            return;
        };
        let Some(&parent_index) = self.layer_index_by_id.get(&parent_id) else {
            return;
        };
        if parent_index >= self.layers.len() || !self.layers[parent_index].live {
            return;
        }
        self.ensure_cache_capacity(parent_index);
        self.cache.parent_layers[layer_index] = Some(parent_index);
        let children = &mut self.cache.child_layers[parent_index];
        if !children.contains(&layer_index) {
            children.push(layer_index);
        }
        children.sort_by_key(|&child_idx| self.layers[child_idx].layer.order);
        self.mark_subtree_dirty(parent_index);
    }

    fn remove_layer_from_adjacency(&mut self, layer_index: usize, parent_layer_id: Option<u64>) {
        let Some(parent_id) = parent_layer_id else {
            return;
        };
        let Some(&parent_index) = self.layer_index_by_id.get(&parent_id) else {
            return;
        };
        if parent_index >= self.cache.child_layers.len() {
            return;
        }
        self.cache.child_layers[parent_index].retain(|&idx| idx != layer_index);
        self.cache.parent_layers[layer_index] = None;
        self.mark_subtree_dirty(parent_index);
    }

    fn insert_object_into_adjacency(&mut self, object_index: usize) {
        if object_index >= self.objects.len() || !self.objects[object_index].live {
            return;
        }
        let layer_id = self.objects[object_index].object.layer_id;
        let Some(&layer_index) = self.layer_index_by_id.get(&layer_id) else {
            return;
        };
        if layer_index >= self.layers.len() || !self.layers[layer_index].live {
            return;
        }
        self.ensure_cache_capacity(layer_index);
        let objects = &mut self.cache.objects_by_layer[layer_index];
        if !objects.contains(&object_index) {
            objects.push(object_index);
        }
        objects.sort_by_key(|&idx| self.objects[idx].object.order);
        self.mark_subtree_dirty(layer_index);
    }

    fn remove_object_from_adjacency(&mut self, object_index: usize, layer_id: u64) {
        let Some(&layer_index) = self.layer_index_by_id.get(&layer_id) else {
            return;
        };
        if layer_index >= self.cache.objects_by_layer.len() {
            return;
        }
        self.cache.objects_by_layer[layer_index].retain(|&idx| idx != object_index);
        self.mark_subtree_dirty(layer_index);
    }

    fn mark_subtree_dirty(&mut self, layer_index: usize) {
        self.ensure_cache_capacity(layer_index);
        let mut current = Some(layer_index);
        while let Some(idx) = current {
            self.cache.subtree_dirty[idx] = true;
            current = self.cache.parent_layers[idx];
        }
        self.cache.draw_ops_dirty = true;
    }

    fn rebuild_subtree_ops_if_needed(&mut self, layer_index: usize) {
        if layer_index >= self.layers.len() || !self.layers[layer_index].live {
            return;
        }
        self.ensure_cache_capacity(layer_index);
        if !self.cache.subtree_dirty[layer_index] {
            return;
        }
        let mut ops: Vec<DrawOp> = Vec::new();
        ops.push(DrawOp::EnterLayer(layer_index));

        let mut items: Vec<(u32, DrawOp)> = Vec::new();
        for &obj_idx in &self.cache.objects_by_layer[layer_index] {
            let order = self.objects[obj_idx].object.order;
            items.push((order, DrawOp::DrawObject(obj_idx)));
        }
        for &child_idx in &self.cache.child_layers[layer_index] {
            let order = self.layers[child_idx].layer.order;
            items.push((order, DrawOp::EnterLayer(child_idx)));
        }
        items.sort_by_key(|(order, _)| *order);

        for (_, op) in items {
            match op {
                DrawOp::DrawObject(object_index) => ops.push(DrawOp::DrawObject(object_index)),
                DrawOp::EnterLayer(child_layer_index) => {
                    self.rebuild_subtree_ops_if_needed(child_layer_index);
                    ops.extend_from_slice(&self.cache.subtree_ops[child_layer_index]);
                }
                DrawOp::ExitLayer(_) => {}
            }
        }
        ops.push(DrawOp::ExitLayer(layer_index));

        self.cache.subtree_ops[layer_index] = ops;
        self.cache.subtree_dirty[layer_index] = false;
    }

    fn refresh_derived_queries_if_needed(&mut self) {
        if !self.cache.derived_dirty {
            return;
        }
        let mut resource_key_count = 0usize;
        let mut clear_packet = None;
        for entry in self.objects.iter().filter(|entry| entry.live) {
            resource_key_count += entry.object.resource_keys.len();
            if clear_packet.is_none() {
                clear_packet = self
                    .packets_for_range(entry.object.packets)
                    .iter()
                    .find_map(|packet| match packet {
                        DrawCommand::Clear(color) => Some(*color),
                        _ => None,
                    });
            }
        }
        self.cache.resource_key_count = resource_key_count;
        self.cache.clear_packet = clear_packet;
        self.cache.derived_dirty = false;
    }

    /// Computes dirty bounds based on the current retained scene (pre-apply).
    /// This mirrors `RenderObjectDelta::dirty_bounds(previous_scene)`, but uses the retained scene.
    #[must_use]
    pub fn dirty_bounds_for_delta(&mut self, delta: &RenderObjectDelta) -> Option<Rect> {
        // Bounds from upserted objects in delta (new bounds).
        let upsert_bounds = delta
            .object_upserts
            .iter()
            .map(|object| object.bounds)
            .reduce(|acc, bounds| acc.union(&bounds));

        // Bounds for objects being removed/reordered should use previous (retained) bounds.
        let reorder_ids = delta
            .object_reorders
            .iter()
            .map(|r| r.object_id)
            .collect::<Vec<_>>();
        let remove_bounds = self.dirty_bounds_for_object_ids(&delta.object_removes);
        let reorder_bounds = self.dirty_bounds_for_object_ids(&reorder_ids);

        // Layer changes can expand affected area (clip/effect chain).
        let layer_upsert_bounds = delta
            .layer_upserts
            .iter()
            .map(|layer| layer.bounds)
            .reduce(|acc, bounds| acc.union(&bounds));
        let layer_reorder_ids = delta
            .layer_reorders
            .iter()
            .map(|r| r.layer_id)
            .collect::<Vec<_>>();
        let layer_remove_bounds = self.dirty_bounds_for_layer_ids(&delta.layer_removes);
        let layer_reorder_bounds = self.dirty_bounds_for_layer_ids(&layer_reorder_ids);

        [
            upsert_bounds,
            remove_bounds,
            reorder_bounds,
            layer_upsert_bounds,
            layer_remove_bounds,
            layer_reorder_bounds,
        ]
        .into_iter()
        .flatten()
        .reduce(|acc, bounds| acc.union(&bounds))
    }

    fn dirty_bounds_for_object_ids(&self, object_ids: &[u64]) -> Option<Rect> {
        object_ids
            .iter()
            .filter_map(|object_id| self.object_index_by_id.get(object_id).copied())
            .filter_map(|idx| self.objects.get(idx))
            .filter(|entry| entry.live)
            .map(|entry| entry.object.bounds)
            .reduce(|acc, bounds| acc.union(&bounds))
    }

    fn dirty_bounds_for_layer_ids(&self, layer_ids: &[u64]) -> Option<Rect> {
        layer_ids
            .iter()
            .filter_map(|layer_id| self.layer_index_by_id.get(layer_id).copied())
            .filter_map(|idx| self.layers.get(idx))
            .filter(|entry| entry.live)
            .map(|entry| entry.layer.bounds)
            .reduce(|acc, bounds| acc.union(&bounds))
    }

    pub fn apply_delta_in_place(&mut self, delta: &RenderObjectDelta) {
        // Removals first.
        for layer_id in &delta.layer_removes {
            if let Some(idx) = self.layer_index_by_id.remove(layer_id) {
                if idx < self.layers.len() {
                    let parent_id = self.layers[idx].layer.parent_layer_id;
                    self.remove_layer_from_adjacency(idx, parent_id);
                    self.layers[idx].live = false;
                    self.free_layer_slots.push(idx);
                    self.live_layers = self.live_layers.saturating_sub(1);
                }
            }
        }
        for object_id in &delta.object_removes {
            if let Some(idx) = self.object_index_by_id.remove(object_id) {
                if idx < self.objects.len() {
                    let layer_id = self.objects[idx].object.layer_id;
                    self.remove_object_from_adjacency(idx, layer_id);
                    self.release_object_packets(idx);
                    self.objects[idx].live = false;
                    self.free_object_slots.push(idx);
                    self.live_objects = self.live_objects.saturating_sub(1);
                    self.cache.derived_dirty = true;
                }
            }
        }

        // Reorders.
        for reorder in &delta.layer_reorders {
            if let Some(&idx) = self.layer_index_by_id.get(&reorder.layer_id) {
                if idx < self.layers.len() && self.layers[idx].live {
                    self.layers[idx].layer.order = reorder.order;
                    self.insert_layer_into_adjacency(idx);
                }
            }
        }
        for reorder in &delta.object_reorders {
            if let Some(&idx) = self.object_index_by_id.get(&reorder.object_id) {
                if idx < self.objects.len() && self.objects[idx].live {
                    self.objects[idx].object.order = reorder.order;
                    self.insert_object_into_adjacency(idx);
                }
            }
        }

        // Upserts.
        for layer in &delta.layer_upserts {
            self.upsert_layer(layer.clone());
        }
        for object in &delta.object_upserts {
            // Normalize packets into retained arena.
            let packets = delta.packets_for_object(object);
            let range = self.allocate_packets(packets);
            let mut normalized = object.clone().with_normalized_packets(range);
            // Ensure the internal staged_packets is cleared (range points into retained packets).
            normalized = normalized.with_normalized_packets(range);
            self.upsert_object(normalized);
        }

        self.size = delta.size;
        self.compact_packets_if_needed();
        self.compact_graph_if_needed();
        self.cache.draw_ops_dirty = true;
        self.cache.derived_dirty = true;
        self.generation += 1;
    }

    fn allocate_packets(&mut self, packets: &[DrawCommand]) -> DrawPacketRange {
        if packets.is_empty() {
            return DrawPacketRange { start: 0, len: 0 };
        }
        if let Some((slot_index, range)) = self
            .free_packet_ranges
            .iter()
            .copied()
            .enumerate()
            .find(|(_, range)| range.len >= packets.len())
        {
            self.packets[range.start..range.start + packets.len()].clone_from_slice(packets);
            self.live_packet_count += packets.len();
            if range.len == packets.len() {
                self.free_packet_ranges.swap_remove(slot_index);
            } else {
                self.free_packet_ranges[slot_index] = DrawPacketRange {
                    start: range.start + packets.len(),
                    len: range.len - packets.len(),
                };
            }
            return DrawPacketRange {
                start: range.start,
                len: packets.len(),
            };
        }

        let start = self.packets.len();
        self.packets.extend_from_slice(packets);
        self.live_packet_count += packets.len();
        DrawPacketRange { start, len: packets.len() }
    }

    fn insert_layer(&mut self, layer: LayerObject) -> usize {
        let idx = self
            .free_layer_slots
            .pop()
            .filter(|&idx| idx < self.layers.len())
            .unwrap_or(self.layers.len());
        self.layer_index_by_id.insert(layer.layer_id, idx);
        if idx == self.layers.len() {
            self.layers.push(LayerEntry { live: true, layer });
        } else {
            self.layers[idx] = LayerEntry { live: true, layer };
        }
        self.live_layers += 1;
        self.insert_layer_into_adjacency(idx);
        idx
    }

    fn upsert_layer(&mut self, layer: LayerObject) {
        match self.layer_index_by_id.get(&layer.layer_id).copied() {
            Some(idx) if idx < self.layers.len() => {
                let old_parent = self.layers[idx].layer.parent_layer_id;
                self.layers[idx].live = true;
                self.layers[idx].layer = layer;
                self.remove_layer_from_adjacency(idx, old_parent);
                self.insert_layer_into_adjacency(idx);
            }
            _ => {
                self.insert_layer(layer);
            }
        }
    }

    fn insert_object_with_range(&mut self, object: RenderObject) -> usize {
        let idx = self
            .free_object_slots
            .pop()
            .filter(|&idx| idx < self.objects.len())
            .unwrap_or(self.objects.len());
        self.object_index_by_id.insert(object.object_id, idx);
        if idx == self.objects.len() {
            self.objects.push(ObjectEntry { live: true, object });
        } else {
            self.objects[idx] = ObjectEntry { live: true, object };
        }
        self.live_objects += 1;
        self.insert_object_into_adjacency(idx);
        self.cache.derived_dirty = true;
        idx
    }

    fn upsert_object(&mut self, object: RenderObject) {
        match self.object_index_by_id.get(&object.object_id).copied() {
            Some(idx) if idx < self.objects.len() => {
                let old_layer_id = self.objects[idx].object.layer_id;
                self.remove_object_from_adjacency(idx, old_layer_id);
                self.release_object_packets(idx);
                self.objects[idx].live = true;
                self.objects[idx].object = object;
                self.insert_object_into_adjacency(idx);
                self.cache.derived_dirty = true;
            }
            _ => {
                self.insert_object_with_range(object);
            }
        }
    }

    fn release_object_packets(&mut self, object_index: usize) {
        if object_index >= self.objects.len() || !self.objects[object_index].live {
            return;
        }
        let range = self.objects[object_index].object.packets;
        if range.len == 0 {
            return;
        }
        self.live_packet_count = self.live_packet_count.saturating_sub(range.len);
        self.free_packet_ranges.push(range);
        self.merge_free_packet_ranges();
    }

    fn merge_free_packet_ranges(&mut self) {
        if self.free_packet_ranges.len() <= 1 {
            return;
        }
        self.free_packet_ranges.sort_by_key(|range| range.start);
        let mut merged: Vec<DrawPacketRange> = Vec::with_capacity(self.free_packet_ranges.len());
        for range in self.free_packet_ranges.drain(..) {
            if let Some(last) = merged.last_mut()
                && last.start + last.len == range.start
            {
                last.len += range.len;
            } else {
                merged.push(range);
            }
        }
        self.free_packet_ranges = merged;
    }

    fn compact_packets_if_needed(&mut self) {
        if self.free_packet_ranges.is_empty() {
            return;
        }
        let free_count: usize = self.free_packet_ranges.iter().map(|range| range.len).sum();
        let should_compact = self.live_packet_count == 0
            || free_count >= self.live_packet_count / 2
            || self.free_packet_ranges.len() > 32;
        if !should_compact {
            return;
        }
        self.compact_packets();
    }

    fn compact_packets(&mut self) {
        let live_objects = self
            .objects
            .iter()
            .filter(|entry| entry.live)
            .map(|entry| entry.object.clone())
            .collect::<Vec<_>>();
        let (packets, compacted_objects) = self.compact_live_objects(live_objects);
        let mut updated_by_id = HashMap::with_capacity(compacted_objects.len());
        for object in compacted_objects {
            updated_by_id.insert(object.object_id, object);
        }
        for entry in &mut self.objects {
            if entry.live && let Some(object) = updated_by_id.remove(&entry.object.object_id) {
                entry.object = object;
            }
        }
        self.packets = packets;
        self.live_packet_count = self.packets.len();
        self.free_packet_ranges.clear();
    }

    fn compact_graph_if_needed(&mut self) {
        let live_layers = self.live_layer_count();
        let live_objects = self.live_object_count();
        let dead_layers = self.layers.len().saturating_sub(live_layers);
        let dead_objects = self.objects.len().saturating_sub(live_objects);
        let should_compact_layers =
            (dead_layers > 0 && dead_layers >= live_layers) || self.free_layer_slots.len() > 64;
        let should_compact_objects =
            (dead_objects > 0 && dead_objects >= live_objects) || self.free_object_slots.len() > 64;
        if !should_compact_layers && !should_compact_objects {
            return;
        }

        let mut new_layers = Vec::with_capacity(live_layers.max(1));
        let mut new_layer_map = HashMap::with_capacity(live_layers.max(1));
        for entry in self.layers.iter().filter(|entry| entry.live) {
            let idx = new_layers.len();
            new_layer_map.insert(entry.layer.layer_id, idx);
            new_layers.push(entry.clone());
        }
        if !new_layer_map.contains_key(&Scene::ROOT_LAYER_ID) {
            let idx = new_layers.len();
            new_layer_map.insert(Scene::ROOT_LAYER_ID, idx);
            new_layers.push(LayerEntry {
                live: true,
                layer: LayerObject::root(self.size),
            });
        }

        let mut new_objects = Vec::with_capacity(live_objects);
        let mut new_object_map = HashMap::with_capacity(live_objects);
        for entry in self.objects.iter().filter(|entry| entry.live) {
            let idx = new_objects.len();
            new_object_map.insert(entry.object.object_id, idx);
            new_objects.push(entry.clone());
        }

        self.layers = new_layers;
        self.objects = new_objects;
        self.layer_index_by_id = new_layer_map;
        self.object_index_by_id = new_object_map;
        self.free_layer_slots.clear();
        self.free_object_slots.clear();
        self.live_layers = self.layers.len();
        self.live_objects = self.objects.len();
        self.rebuild_adjacency_from_storage();
        self.cache.derived_dirty = true;
    }

    fn compact_live_objects(
        &self,
        objects: Vec<RenderObject>,
    ) -> (Vec<DrawCommand>, Vec<RenderObject>) {
        let mut packets = Vec::with_capacity(self.live_packet_count);
        let normalized = objects
            .into_iter()
            .map(|object| {
                let object_packets = self.packets_for_range(object.packets).to_vec();
                let start = packets.len();
                packets.extend_from_slice(&object_packets);
                object.with_normalized_packets(DrawPacketRange {
                    start,
                    len: object_packets.len(),
                })
            })
            .collect();
        (packets, normalized)
    }

    fn packets_for_range(&self, range: DrawPacketRange) -> &[DrawCommand] {
        &self.packets[range.start..range.start + range.len]
    }
}

#[cfg(test)]
mod tests {
    use super::RetainedScene;
    use crate::{
        Brush, DrawCommand, DrawOp, DrawPacketRange, LayerObject, RenderObject,
        RenderObjectDelta, Scene, SceneBlendMode, SceneResourceKey, SceneTransform, Shape,
    };
    use zeno_core::{Color, Rect, Size};

    fn rect_object(object_id: u64, order: u32, width: f32) -> RenderObject {
        RenderObject::new(
            object_id,
            Scene::ROOT_LAYER_ID,
            order,
            Rect::new(0.0, 0.0, width, 10.0),
            SceneTransform::identity(),
            None,
            vec![DrawCommand::Fill {
                shape: Shape::Rect(Rect::new(0.0, 0.0, width, 10.0)),
                brush: Brush::Solid(Color::WHITE),
            }],
        )
    }

    fn delta(size: Size) -> RenderObjectDelta {
        RenderObjectDelta {
            size,
            packets: Vec::new(),
            base_layer_count: 1,
            base_object_count: 0,
            layer_upserts: Vec::new(),
            layer_reorders: Vec::new(),
            layer_removes: Vec::new(),
            object_upserts: Vec::new(),
            object_reorders: Vec::new(),
            object_removes: Vec::new(),
        }
    }

    fn child_layer(layer_id: u64, owner_object_id: u64, order: u32) -> LayerObject {
        LayerObject::new(
            layer_id,
            owner_object_id,
            Some(Scene::ROOT_LAYER_ID),
            order,
            Rect::new(0.0, 0.0, 20.0, 20.0),
            Rect::new(0.0, 0.0, 20.0, 20.0),
            SceneTransform::identity(),
            None,
            1.0,
            SceneBlendMode::Normal,
            Vec::new(),
            false,
        )
    }

    fn draw_op_trace(scene: &mut RetainedScene) -> Vec<String> {
        let ops = scene.draw_ops().to_vec();
        ops.iter()
            .map(|op| match *op {
                DrawOp::EnterLayer(idx) => format!("enter:{}", scene.layer(idx).layer_id),
                DrawOp::DrawObject(idx) => format!("object:{}", scene.object(idx).object_id),
                DrawOp::ExitLayer(idx) => format!("exit:{}", scene.layer(idx).layer_id),
            })
            .collect()
    }

    #[test]
    fn removing_object_releases_packet_range() {
        let size = Size::new(100.0, 50.0);
        let scene = Scene::from_objects(size, None, vec![rect_object(1, 0, 10.0)]);
        let mut retained = RetainedScene::from_scene(scene);

        let mut patch = delta(size);
        patch.base_object_count = retained.live_object_count();
        patch.object_removes.push(1);
        retained.apply_delta_in_place(&patch);

        assert_eq!(retained.live_object_count(), 0);
        assert_eq!(retained.live_packet_count, 0);
        assert_eq!(retained.packets.len(), 0);
        assert!(retained.free_packet_ranges.is_empty());
    }

    #[test]
    fn upsert_reuses_free_packet_range_without_growing_arena() {
        let size = Size::new(100.0, 50.0);
        let scene = Scene::from_objects(
            size,
            None,
            vec![
                RenderObject::new(
                    1,
                    Scene::ROOT_LAYER_ID,
                    0,
                    Rect::new(0.0, 0.0, 20.0, 10.0),
                    SceneTransform::identity(),
                    None,
                    vec![
                        DrawCommand::Fill {
                            shape: Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
                            brush: Brush::Solid(Color::WHITE),
                        },
                        DrawCommand::Fill {
                            shape: Shape::Rect(Rect::new(10.0, 0.0, 10.0, 10.0)),
                            brush: Brush::Solid(Color::WHITE),
                        },
                    ],
                ),
                rect_object(2, 1, 10.0),
            ],
        );
        let mut retained = RetainedScene::from_scene(scene);
        let arena_len_before = retained.packets.len();

        let mut remove_patch = delta(size);
        remove_patch.base_object_count = retained.live_object_count();
        remove_patch.object_removes.push(1);
        retained.apply_delta_in_place(&remove_patch);
        let arena_len_after_remove = retained.packets.len();

        let mut upsert_patch = delta(size);
        upsert_patch.base_object_count = retained.live_object_count();
        upsert_patch.packets = vec![DrawCommand::Fill {
            shape: Shape::Rect(Rect::new(0.0, 0.0, 8.0, 8.0)),
            brush: Brush::Solid(Color::WHITE),
        }];
        upsert_patch.object_upserts.push(
            RenderObject::new(
                3,
                Scene::ROOT_LAYER_ID,
                2,
                Rect::new(0.0, 0.0, 8.0, 8.0),
                SceneTransform::identity(),
                None,
                upsert_patch.packets.clone(),
            )
            .with_normalized_packets(DrawPacketRange { start: 0, len: 1 }),
        );
        retained.apply_delta_in_place(&upsert_patch);

        assert!(retained.packets.len() <= arena_len_before);
        assert_eq!(retained.packets.len(), arena_len_after_remove + 1);
        assert_eq!(retained.live_packet_count, 2);
        let inserted = retained.object(retained.object_index_by_id[&3]).clone();
        assert_eq!(inserted.packets.start, 1);
        assert_eq!(inserted.packets.len, 1);
        assert!(retained.free_packet_ranges.is_empty());
    }

    #[test]
    fn fragmentation_threshold_compacts_packet_arena() {
        let size = Size::new(100.0, 50.0);
        let scene = Scene::from_objects(
            size,
            None,
            vec![rect_object(1, 0, 10.0), rect_object(2, 1, 10.0), rect_object(3, 2, 10.0)],
        );
        let mut retained = RetainedScene::from_scene(scene);

        let mut patch = delta(size);
        patch.base_object_count = retained.live_object_count();
        patch.object_removes.extend([1, 2]);
        retained.apply_delta_in_place(&patch);

        assert_eq!(retained.live_object_count(), 1);
        assert_eq!(retained.live_packet_count, 1);
        assert_eq!(retained.packets.len(), 1);
        assert!(retained.free_packet_ranges.is_empty());
        let remaining = retained.object(retained.object_index_by_id[&3]).clone();
        assert_eq!(remaining.packets, DrawPacketRange { start: 0, len: 1 });
    }

    #[test]
    fn removed_object_slot_is_reused_before_graph_compaction() {
        let size = Size::new(100.0, 50.0);
        let scene = Scene::from_objects(
            size,
            None,
            vec![rect_object(1, 0, 10.0), rect_object(2, 1, 10.0), rect_object(3, 2, 10.0)],
        );
        let mut retained = RetainedScene::from_scene(scene);

        let mut patch = delta(size);
        patch.base_object_count = retained.live_object_count();
        patch.object_removes.push(2);
        retained.apply_delta_in_place(&patch);

        let freed_slot = retained.free_object_slots[0];
        let mut upsert = delta(size);
        upsert.base_object_count = retained.live_object_count();
        upsert.packets = vec![DrawCommand::Fill {
            shape: Shape::Rect(Rect::new(0.0, 0.0, 12.0, 10.0)),
            brush: Brush::Solid(Color::WHITE),
        }];
        upsert.object_upserts.push(
            RenderObject::new(
                4,
                Scene::ROOT_LAYER_ID,
                3,
                Rect::new(0.0, 0.0, 12.0, 10.0),
                SceneTransform::identity(),
                None,
                upsert.packets.clone(),
            )
            .with_normalized_packets(DrawPacketRange { start: 0, len: 1 }),
        );
        retained.apply_delta_in_place(&upsert);

        assert_eq!(retained.object_index_by_id[&4], freed_slot);
        assert!(retained.free_object_slots.is_empty());
    }

    #[test]
    fn graph_fragmentation_threshold_compacts_object_slots() {
        let size = Size::new(100.0, 50.0);
        let scene = Scene::from_objects(
            size,
            None,
            vec![rect_object(1, 0, 10.0), rect_object(2, 1, 10.0), rect_object(3, 2, 10.0)],
        );
        let mut retained = RetainedScene::from_scene(scene);

        let mut patch = delta(size);
        patch.base_object_count = retained.live_object_count();
        patch.object_removes.extend([1, 2]);
        retained.apply_delta_in_place(&patch);

        assert_eq!(retained.objects.len(), 1);
        assert_eq!(retained.object_index_by_id[&3], 0);
        assert!(retained.free_object_slots.is_empty());
    }

    #[test]
    fn removed_layer_slot_is_reused_before_graph_compaction() {
        let size = Size::new(100.0, 50.0);
        let scene = Scene::from_layers_and_objects(
            size,
            None,
            vec![
                LayerObject::root(size),
                child_layer(10, 10, 1),
                child_layer(20, 20, 2),
            ],
            Vec::new(),
        );
        let mut retained = RetainedScene::from_scene(scene);

        let mut patch = delta(size);
        patch.base_layer_count = retained.live_layer_count();
        patch.layer_removes.push(10);
        retained.apply_delta_in_place(&patch);

        let freed_slot = retained.free_layer_slots[0];
        let mut upsert = delta(size);
        upsert.base_layer_count = retained.live_layer_count();
        upsert.layer_upserts.push(child_layer(30, 30, 3));
        retained.apply_delta_in_place(&upsert);

        assert_eq!(retained.layer_index_by_id[&30], freed_slot);
        assert!(retained.free_layer_slots.is_empty());
    }

    #[test]
    fn graph_fragmentation_threshold_compacts_layer_slots() {
        let size = Size::new(100.0, 50.0);
        let scene = Scene::from_layers_and_objects(
            size,
            None,
            vec![
                LayerObject::root(size),
                child_layer(10, 10, 1),
                child_layer(20, 20, 2),
                child_layer(30, 30, 3),
            ],
            Vec::new(),
        );
        let mut retained = RetainedScene::from_scene(scene);

        let mut patch = delta(size);
        patch.base_layer_count = retained.live_layer_count();
        patch.layer_removes.extend([10, 20]);
        retained.apply_delta_in_place(&patch);

        assert_eq!(retained.layers.len(), 2);
        assert_eq!(retained.live_layer_count(), 2);
        assert_eq!(retained.layer_index_by_id[&Scene::ROOT_LAYER_ID], 0);
        assert_eq!(retained.layer_index_by_id[&30], 1);
        assert!(retained.free_layer_slots.is_empty());
    }

    #[test]
    fn draw_ops_keep_stable_traversal_after_local_subtree_reorder() {
        let size = Size::new(100.0, 50.0);
        let scene = Scene::from_layers_and_objects(
            size,
            None,
            vec![
                LayerObject::root(size),
                child_layer(10, 10, 1),
                child_layer(20, 20, 2),
            ],
            vec![
                rect_object(1, 0, 10.0),
                RenderObject::new(
                    2,
                    10,
                    0,
                    Rect::new(0.0, 0.0, 8.0, 8.0),
                    SceneTransform::identity(),
                    None,
                    vec![DrawCommand::Fill {
                        shape: Shape::Rect(Rect::new(0.0, 0.0, 8.0, 8.0)),
                        brush: Brush::Solid(Color::WHITE),
                    }],
                ),
                RenderObject::new(
                    3,
                    10,
                    1,
                    Rect::new(0.0, 0.0, 6.0, 6.0),
                    SceneTransform::identity(),
                    None,
                    vec![DrawCommand::Fill {
                        shape: Shape::Rect(Rect::new(0.0, 0.0, 6.0, 6.0)),
                        brush: Brush::Solid(Color::WHITE),
                    }],
                ),
            ],
        );
        let mut retained = RetainedScene::from_scene(scene);

        assert_eq!(
            draw_op_trace(&mut retained),
            vec![
                "enter:0", "object:1", "enter:10", "object:2", "object:3", "exit:10", "enter:20",
                "exit:20", "exit:0",
            ]
        );

        let mut patch = delta(size);
        patch.base_layer_count = retained.live_layer_count();
        patch.base_object_count = retained.live_object_count();
        patch.object_reorders.push(crate::RenderObjectOrder {
            object_id: 3,
            order: 0,
        });
        patch.object_reorders.push(crate::RenderObjectOrder {
            object_id: 2,
            order: 1,
        });
        retained.apply_delta_in_place(&patch);

        assert_eq!(
            draw_op_trace(&mut retained),
            vec![
                "enter:0", "object:1", "enter:10", "object:3", "object:2", "exit:10", "enter:20",
                "exit:20", "exit:0",
            ]
        );
    }

    #[test]
    fn derived_query_cache_refreshes_after_object_mutation() {
        let size = Size::new(100.0, 50.0);
        let mut clear_object = RenderObject::new(
            1,
            Scene::ROOT_LAYER_ID,
            0,
            Rect::new(0.0, 0.0, 10.0, 10.0),
            SceneTransform::identity(),
            None,
            vec![DrawCommand::Clear(Color::WHITE)],
        );
        clear_object.resource_keys = vec![SceneResourceKey(11), SceneResourceKey(12)];
        let mut fill_object = rect_object(2, 1, 10.0);
        fill_object.resource_keys = vec![SceneResourceKey(21)];
        let scene = Scene::from_objects(size, Some(Color::WHITE), vec![clear_object, fill_object]);
        let mut retained = RetainedScene::from_scene(scene);

        assert_eq!(retained.resource_key_count(), 3);
        assert_eq!(retained.clear_packet(), Some(Color::WHITE));
        assert_eq!(retained.packet_count(), 2);

        let mut patch = delta(size);
        patch.base_object_count = retained.live_object_count();
        patch.object_removes.push(1);
        retained.apply_delta_in_place(&patch);

        assert_eq!(retained.resource_key_count(), 1);
        assert_eq!(retained.clear_packet(), None);
        assert_eq!(retained.packet_count(), 2);
    }
}
