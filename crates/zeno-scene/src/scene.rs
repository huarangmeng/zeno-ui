use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use zeno_core::{Color, Point, Rect, Size, Transform2D};
use zeno_text::TextLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneResourceKey(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub enum Brush {
    Solid(Color),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Rect(Rect),
    RoundedRect { rect: Rect, radius: f32 },
    Circle { center: Point, radius: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    Fill { shape: Shape, brush: Brush },
    Stroke { shape: Shape, stroke: Stroke },
    Text {
        position: Point,
        layout: TextLayout,
        color: Color,
    },
    Clear(Color),
}

impl DrawCommand {
    #[must_use]
    pub fn resource_key(&self) -> Option<SceneResourceKey> {
        match self {
            Self::Text { layout, .. } => Some(SceneResourceKey(layout.cache_key().stable_hash())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub size: Size,
    pub clear_color: Option<Color>,
    pub packets: Vec<DrawCommand>,
    pub layer_graph: Vec<LayerObject>,
    pub objects: Vec<RenderObject>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawPacketRange {
    pub start: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerObject {
    pub layer_id: u64,
    pub owner_object_id: u64,
    pub parent_layer_id: Option<u64>,
    pub order: u32,
    pub local_bounds: Rect,
    pub bounds: Rect,
    pub transform: SceneTransform,
    pub clip: Option<SceneClip>,
    pub opacity: f32,
    pub blend_mode: SceneBlendMode,
    pub effects: Vec<SceneEffect>,
    pub offscreen: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderObject {
    pub object_id: u64,
    pub layer_id: u64,
    pub order: u32,
    pub bounds: Rect,
    pub transform: SceneTransform,
    pub clip: Option<SceneClip>,
    pub packets: DrawPacketRange,
    pub packet_count: usize,
    pub packet_signature: u64,
    pub resource_keys: Vec<SceneResourceKey>,
    staged_packets: Option<Vec<DrawCommand>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderObjectDelta {
    pub size: Size,
    pub packets: Vec<DrawCommand>,
    pub base_layer_count: usize,
    pub base_object_count: usize,
    pub layer_upserts: Vec<LayerObject>,
    pub layer_reorders: Vec<LayerOrder>,
    pub layer_removes: Vec<u64>,
    pub object_upserts: Vec<RenderObject>,
    pub object_reorders: Vec<RenderObjectOrder>,
    pub object_removes: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerOrder {
    pub layer_id: u64,
    pub order: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderObjectOrder {
    pub object_id: u64,
    pub order: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderSceneUpdate {
    Full(Scene),
    Delta {
        delta: RenderObjectDelta,
        current: Scene,
    },
}

pub type SceneTransform = Transform2D;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneBlendMode {
    Normal,
    Multiply,
    Screen,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneClip {
    Rect(Rect),
    RoundedRect { rect: Rect, radius: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SceneEffect {
    Blur { sigma: f32 },
    DropShadow {
        dx: f32,
        dy: f32,
        blur: f32,
        color: Color,
    },
}

impl Scene {
    pub const ROOT_LAYER_ID: u64 = 0;

    #[must_use]
    pub fn new(size: Size) -> Self {
        Self {
            size,
            clear_color: None,
            packets: Vec::new(),
            layer_graph: vec![LayerObject::root(size)],
            objects: Vec::new(),
        }
    }

    #[must_use]
    pub fn from_objects(
        size: Size,
        clear_color: Option<Color>,
        objects: Vec<RenderObject>,
    ) -> Self {
        Self::from_layers_and_objects(size, clear_color, vec![LayerObject::root(size)], objects)
    }

    #[must_use]
    pub fn from_layers_and_objects(
        size: Size,
        clear_color: Option<Color>,
        layers: Vec<LayerObject>,
        objects: Vec<RenderObject>,
    ) -> Self {
        let (packets, objects) = Self::compact_objects(objects);
        Self::from_layers_and_objects_with_packets(size, clear_color, layers, packets, objects)
    }

    #[must_use]
    pub fn from_layers_and_objects_with_packets(
        size: Size,
        clear_color: Option<Color>,
        mut layers: Vec<LayerObject>,
        packets: Vec<DrawCommand>,
        objects: Vec<RenderObject>,
    ) -> Self {
        if !layers.iter().any(|layer| layer.layer_id == Self::ROOT_LAYER_ID) {
            layers.push(LayerObject::root(size));
        }
        layers.sort_by_key(|layer| layer.order);
        Self {
            size,
            clear_color,
            packets,
            layer_graph: layers,
            objects,
        }
    }

    pub fn push(&mut self, packet: DrawCommand) {
        self.layer_graph = vec![LayerObject::root(self.size)];
        let start = self.packets.len();
        let resource_keys = packet.resource_key().into_iter().collect();
        let signature = packet_signature(std::slice::from_ref(&packet));
        self.packets.push(packet);
        self.objects.push(RenderObject::with_range(
            u64::MAX - self.objects.len() as u64,
            Self::ROOT_LAYER_ID,
            self.objects.len() as u32,
            Rect::new(0.0, 0.0, self.size.width, self.size.height),
            Transform2D::identity(),
            None,
            DrawPacketRange { start, len: 1 },
            1,
            signature,
            resource_keys,
        ));
    }

    #[must_use]
    pub fn iter_packets(&self) -> impl Iterator<Item = &DrawCommand> {
        self.packets.iter()
    }

    #[must_use]
    pub fn packet_count(&self) -> usize {
        self.packets.len() + usize::from(self.clear_color.is_some() && self.clear_packet().is_none())
    }

    #[must_use]
    pub fn clear_packet(&self) -> Option<Color> {
        self.iter_packets().find_map(|packet| match packet {
            DrawCommand::Clear(color) => Some(*color),
            _ => None,
        })
    }

    #[must_use]
    pub fn resource_keys(&self) -> Vec<SceneResourceKey> {
        self.objects
            .iter()
            .flat_map(|object| object.resource_keys.iter().copied())
            .collect()
    }

    #[must_use]
    pub fn packets_for_object<'a>(&'a self, object: &'a RenderObject) -> &'a [DrawCommand] {
        &self.packets[object.packets.start..object.packets.start + object.packets.len]
    }

    #[must_use]
    pub fn apply_delta(&self, delta: &RenderObjectDelta) -> Self {
        let mut layers: Vec<LayerObject> = self
            .layer_graph
            .iter()
            .filter(|layer| !delta.layer_removes.contains(&layer.layer_id))
            .cloned()
            .collect();
        for upsert in &delta.layer_upserts {
            if let Some(existing) = layers.iter_mut().find(|layer| layer.layer_id == upsert.layer_id) {
                *existing = upsert.clone();
            } else {
                layers.push(upsert.clone());
            }
        }
        for reorder in &delta.layer_reorders {
            if let Some(existing) = layers.iter_mut().find(|layer| layer.layer_id == reorder.layer_id) {
                existing.order = reorder.order;
            }
        }
        layers.sort_by_key(|layer| layer.order);

        let mut objects: Vec<(RenderObject, bool)> = self
            .objects
            .iter()
            .filter(|object| !delta.object_removes.contains(&object.object_id))
            .cloned()
            .map(|object| (object, false))
            .collect();

        for upsert in &delta.object_upserts {
            if let Some(existing) = objects
                .iter_mut()
                .find(|(object, _)| object.object_id == upsert.object_id)
            {
                *existing = (upsert.clone(), true);
            } else {
                objects.push((upsert.clone(), true));
            }
        }
        for reorder in &delta.object_reorders {
            if let Some(existing) = objects
                .iter_mut()
                .find(|(object, _)| object.object_id == reorder.object_id)
            {
                existing.0.order = reorder.order;
            }
        }

        objects.sort_by_key(|(object, _)| object.order);
        let mut packets = Vec::new();
        let rebuilt_objects = objects
            .into_iter()
            .map(|(object, from_delta)| {
                let object_packets = if from_delta {
                    delta.packets_for_object(&object).to_vec()
                } else {
                    self.packets_for_object(&object).to_vec()
                };
                let start = packets.len();
                packets.extend_from_slice(&object_packets);
                object.with_normalized_packets(DrawPacketRange {
                    start,
                    len: object_packets.len(),
                })
            })
            .collect();
        Self::from_layers_and_objects_with_packets(
            delta.size,
            self.clear_color,
            layers,
            packets,
            rebuilt_objects,
        )
    }

    #[must_use]
    pub fn dirty_bounds_for_objects(&self, object_ids: &[u64]) -> Option<Rect> {
        self.objects
            .iter()
            .filter(|object| object_ids.contains(&object.object_id))
            .filter_map(|object| self.effective_bounds_for_object(object))
            .reduce(|acc, bounds| acc.union(&bounds))
    }

    #[must_use]
    pub fn dirty_bounds_for_layers(&self, layer_ids: &[u64]) -> Option<Rect> {
        self.layer_graph
            .iter()
            .filter(|layer| layer_ids.contains(&layer.layer_id))
            .filter_map(|layer| self.effective_bounds_for_layer(layer.layer_id))
            .reduce(|acc, bounds| acc.union(&bounds))
    }

    #[must_use]
    pub fn effective_bounds_for_layer(&self, layer_id: u64) -> Option<Rect> {
        let layer = self.layer_graph.iter().find(|layer| layer.layer_id == layer_id)?;
        self.effective_clip_bounds_for_layer(layer_id)
            .map_or(Some(layer.bounds), |clip_bounds| rect_intersection(layer.bounds, clip_bounds))
    }

    #[must_use]
    pub fn effective_bounds_for_object(&self, object: &RenderObject) -> Option<Rect> {
        let clipped = self
            .effective_clip_bounds_for_layer(object.layer_id)
            .map_or(Some(object.bounds), |clip_bounds| {
                rect_intersection(object.bounds, clip_bounds)
            })?;
        object.clip.map_or(Some(clipped), |clip| {
            let layer_transform = self.layer_world_transform(object.layer_id)?;
            rect_intersection(clipped, layer_transform.map_rect(scene_clip_bounds(clip)))
        })
    }

    #[must_use]
    pub fn effective_clip_bounds_for_layer(&self, layer_id: u64) -> Option<Rect> {
        let mut current_layer_id = Some(layer_id);
        let mut clip_bounds = None;
        while let Some(id) = current_layer_id {
            let layer = self.layer_graph.iter().find(|layer| layer.layer_id == id)?;
            if let Some(clip) = layer.clip {
                let world_bounds = self.layer_world_transform(id)?.map_rect(scene_clip_bounds(clip));
                clip_bounds = match clip_bounds {
                    Some(existing) => rect_intersection(existing, world_bounds),
                    None => Some(world_bounds),
                };
            }
            current_layer_id = layer.parent_layer_id;
        }
        clip_bounds
    }

    fn layer_world_transform(&self, layer_id: u64) -> Option<Transform2D> {
        let layer = self.layer_graph.iter().find(|layer| layer.layer_id == layer_id)?;
        layer.parent_layer_id.map_or_else(
            || Some(layer.transform),
            |parent_id| {
                self.layer_world_transform(parent_id)
                    .map(|transform| transform.then(layer.transform))
            },
        )
    }

    #[must_use]
    pub fn compact_objects(objects: Vec<RenderObject>) -> (Vec<DrawCommand>, Vec<RenderObject>) {
        let mut packets = Vec::new();
        let normalized = objects
            .into_iter()
            .map(|object| {
                let object_packets = object.staged_packets.clone().unwrap_or_default();
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
}

fn scene_clip_bounds(clip: SceneClip) -> Rect {
    match clip {
        SceneClip::Rect(rect) => rect,
        SceneClip::RoundedRect { rect, .. } => rect,
    }
}

fn rect_intersection(a: Rect, b: Rect) -> Option<Rect> {
    if !a.intersects(&b) {
        return None;
    }
    let left = a.origin.x.max(b.origin.x);
    let top = a.origin.y.max(b.origin.y);
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    Some(Rect::new(left, top, right - left, bottom - top))
}

fn packet_signature(packets: &[DrawCommand]) -> u64 {
    let mut hasher = DefaultHasher::new();
    packets.len().hash(&mut hasher);
    for packet in packets {
        match packet {
            DrawCommand::Fill { shape, brush } => {
                1u8.hash(&mut hasher);
                hash_shape(shape, &mut hasher);
                hash_brush(brush, &mut hasher);
            }
            DrawCommand::Stroke { shape, stroke } => {
                2u8.hash(&mut hasher);
                hash_shape(shape, &mut hasher);
                hash_f32(stroke.width, &mut hasher);
                hash_color(stroke.color, &mut hasher);
            }
            DrawCommand::Text {
                position,
                layout,
                color,
            } => {
                3u8.hash(&mut hasher);
                hash_point(*position, &mut hasher);
                layout.cache_key().stable_hash().hash(&mut hasher);
                hash_color(*color, &mut hasher);
            }
            DrawCommand::Clear(color) => {
                4u8.hash(&mut hasher);
                hash_color(*color, &mut hasher);
            }
        }
    }
    hasher.finish()
}

fn hash_shape(shape: &Shape, hasher: &mut DefaultHasher) {
    match shape {
        Shape::Rect(rect) => {
            1u8.hash(hasher);
            hash_rect(*rect, hasher);
        }
        Shape::RoundedRect { rect, radius } => {
            2u8.hash(hasher);
            hash_rect(*rect, hasher);
            hash_f32(*radius, hasher);
        }
        Shape::Circle { center, radius } => {
            3u8.hash(hasher);
            hash_point(*center, hasher);
            hash_f32(*radius, hasher);
        }
    }
}

fn hash_brush(brush: &Brush, hasher: &mut DefaultHasher) {
    match brush {
        Brush::Solid(color) => {
            1u8.hash(hasher);
            hash_color(*color, hasher);
        }
    }
}

fn hash_rect(rect: Rect, hasher: &mut DefaultHasher) {
    hash_point(rect.origin, hasher);
    hash_f32(rect.size.width, hasher);
    hash_f32(rect.size.height, hasher);
}

fn hash_point(point: Point, hasher: &mut DefaultHasher) {
    hash_f32(point.x, hasher);
    hash_f32(point.y, hasher);
}

fn hash_color(color: Color, hasher: &mut DefaultHasher) {
    color.red.hash(hasher);
    color.green.hash(hasher);
    color.blue.hash(hasher);
    color.alpha.hash(hasher);
}

fn hash_f32(value: f32, hasher: &mut DefaultHasher) {
    value.to_bits().hash(hasher);
}

impl LayerObject {
    #[must_use]
    pub fn root(size: Size) -> Self {
        Self {
            layer_id: Scene::ROOT_LAYER_ID,
            owner_object_id: Scene::ROOT_LAYER_ID,
            parent_layer_id: None,
            order: 0,
            local_bounds: Rect::new(0.0, 0.0, size.width, size.height),
            bounds: Rect::new(0.0, 0.0, size.width, size.height),
            transform: Transform2D::identity(),
            clip: None,
            opacity: 1.0,
            blend_mode: SceneBlendMode::Normal,
            effects: Vec::new(),
            offscreen: false,
        }
    }

    #[must_use]
    pub fn new(
        layer_id: u64,
        owner_object_id: u64,
        parent_layer_id: Option<u64>,
        order: u32,
        local_bounds: Rect,
        bounds: Rect,
        transform: SceneTransform,
        clip: Option<SceneClip>,
        opacity: f32,
        blend_mode: SceneBlendMode,
        effects: Vec<SceneEffect>,
        offscreen: bool,
    ) -> Self {
        Self {
            layer_id,
            owner_object_id,
            parent_layer_id,
            order,
            local_bounds,
            bounds,
            transform,
            clip,
            opacity,
            blend_mode,
            effects,
            offscreen,
        }
    }
}

impl RenderObject {
    #[must_use]
    pub fn new(
        object_id: u64,
        layer_id: u64,
        order: u32,
        bounds: Rect,
        transform: SceneTransform,
        clip: Option<SceneClip>,
        packets: Vec<DrawCommand>,
    ) -> Self {
        Self::from_packets(object_id, layer_id, order, bounds, transform, clip, packets)
    }

    #[must_use]
    pub fn with_range(
        object_id: u64,
        layer_id: u64,
        order: u32,
        bounds: Rect,
        transform: SceneTransform,
        clip: Option<SceneClip>,
        packets: DrawPacketRange,
        packet_count: usize,
        packet_signature: u64,
        resource_keys: Vec<SceneResourceKey>,
    ) -> Self {
        Self {
            object_id,
            layer_id,
            order,
            bounds,
            transform,
            clip,
            packets,
            packet_count,
            packet_signature,
            resource_keys,
            staged_packets: None,
        }
    }

    #[must_use]
    pub fn from_packets(
        object_id: u64,
        layer_id: u64,
        order: u32,
        bounds: Rect,
        transform: SceneTransform,
        clip: Option<SceneClip>,
        packets: Vec<DrawCommand>,
    ) -> Self {
        let packet_count = packets.len();
        let packet_signature = packet_signature(&packets);
        let resource_keys = packets.iter().filter_map(DrawCommand::resource_key).collect();
        Self {
            object_id,
            layer_id,
            order,
            bounds,
            transform,
            clip,
            packets: DrawPacketRange {
                start: 0,
                len: packet_count,
            },
            packet_count,
            packet_signature,
            resource_keys,
            staged_packets: Some(packets),
        }
    }

    #[must_use]
    pub fn with_normalized_packets(mut self, range: DrawPacketRange) -> Self {
        self.packets = range;
        self.packet_count = range.len;
        self.staged_packets = None;
        self
    }
}

impl RenderObjectDelta {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layer_upserts.is_empty()
            && self.layer_reorders.is_empty()
            && self.layer_removes.is_empty()
            && self.object_upserts.is_empty()
            && self.object_reorders.is_empty()
            && self.object_removes.is_empty()
    }

    #[must_use]
    pub fn dirty_bounds(&self, previous: Option<&Scene>) -> Option<Rect> {
        let upsert_bounds = self
            .object_upserts
            .iter()
            .map(|object| object.bounds)
            .reduce(|acc, bounds| acc.union(&bounds));
        let affected_layer_bounds = previous.and_then(|scene| {
            let mut layer_ids: Vec<u64> = self
                .object_upserts
                .iter()
                .map(|object| object.layer_id)
                .filter(|layer_id| *layer_id != Scene::ROOT_LAYER_ID)
                .collect();
            layer_ids.extend(
                scene
                    .objects
                    .iter()
                    .filter(|object| self.object_removes.contains(&object.object_id))
                    .map(|object| object.layer_id)
                    .filter(|layer_id| *layer_id != Scene::ROOT_LAYER_ID),
            );
            layer_ids.sort_unstable();
            layer_ids.dedup();
            scene.dirty_bounds_for_layers(&layer_ids)
        });
        let previous_upsert_bounds = previous.and_then(|scene| {
            let object_ids: Vec<u64> = self.object_upserts.iter().map(|object| object.object_id).collect();
            scene.dirty_bounds_for_objects(&object_ids)
        });
        let remove_bounds = previous.and_then(|scene| scene.dirty_bounds_for_objects(&self.object_removes));
        let reorder_bounds = previous.and_then(|scene| {
            let object_ids: Vec<u64> = self
                .object_reorders
                .iter()
                .map(|reorder| reorder.object_id)
                .collect();
            scene.dirty_bounds_for_objects(&object_ids)
        });
        let layer_upsert_bounds = self
            .layer_upserts
            .iter()
            .map(|layer| layer.bounds)
            .reduce(|acc, bounds| acc.union(&bounds));
        let layer_reorder_bounds = previous.and_then(|scene| {
            let layer_ids: Vec<u64> = self
                .layer_reorders
                .iter()
                .map(|reorder| reorder.layer_id)
                .collect();
            scene.dirty_bounds_for_layers(&layer_ids)
        });
        let previous_layer_bounds = previous.and_then(|scene| {
            let layer_ids: Vec<u64> = self.layer_upserts.iter().map(|layer| layer.layer_id).collect();
            scene.dirty_bounds_for_layers(&layer_ids)
        });
        let layer_remove_bounds = previous.and_then(|scene| scene.dirty_bounds_for_layers(&self.layer_removes));
        [
            upsert_bounds,
            affected_layer_bounds,
            previous_upsert_bounds,
            remove_bounds,
            reorder_bounds,
            layer_upsert_bounds,
            layer_reorder_bounds,
            previous_layer_bounds,
            layer_remove_bounds,
        ]
        .into_iter()
        .flatten()
        .reduce(|acc, bounds| acc.union(&bounds))
    }

    #[must_use]
    pub fn packets_for_object<'a>(&'a self, object: &'a RenderObject) -> &'a [DrawCommand] {
        if let Some(packets) = object.staged_packets.as_ref() {
            return packets.as_slice();
        }
        &self.packets[object.packets.start..object.packets.start + object.packets.len]
    }
}

impl RenderSceneUpdate {
    #[must_use]
    pub fn snapshot(&self, previous: Option<&Scene>) -> Option<Scene> {
        match self {
            Self::Full(scene) => Some(scene.clone()),
            Self::Delta { delta, current } => previous.map_or_else(
                || Some(current.clone()),
                |scene| Some(scene.apply_delta(delta)),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanvasOp {
    Save,
    Restore,
    Translate { x: f32, y: f32 },
    ClipRect(Rect),
    ClipRoundedRect { rect: Rect, radius: f32 },
}
