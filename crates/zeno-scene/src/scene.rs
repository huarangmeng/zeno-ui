use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::display_list::DisplayImage;
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
    Fill {
        shape: Shape,
        brush: Brush,
    },
    Stroke {
        shape: Shape,
        stroke: Stroke,
    },
    Text {
        position: Point,
        layout: TextLayout,
        color: Color,
    },
    Image {
        image: DisplayImage,
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
        if !layers
            .iter()
            .any(|layer| layer.layer_id == Self::ROOT_LAYER_ID)
        {
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
        self.packets.len()
            + usize::from(self.clear_color.is_some() && self.clear_packet().is_none())
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
            DrawCommand::Image { image } => {
                5u8.hash(&mut hasher);
                hash_rect(image.dest_rect, &mut hasher);
                image.width.hash(&mut hasher);
                image.height.hash(&mut hasher);
                image.rgba8.hash(&mut hasher);
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
        let resource_keys = packets
            .iter()
            .filter_map(DrawCommand::resource_key)
            .collect();
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanvasOp {
    Save,
    Restore,
    Translate { x: f32, y: f32 },
    ClipRect(Rect),
    ClipRoundedRect { rect: Rect, radius: f32 },
}
