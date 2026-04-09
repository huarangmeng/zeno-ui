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
    pub commands: Vec<DrawCommand>,
    pub layers: Vec<SceneLayer>,
    pub blocks: Vec<SceneBlock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandRange {
    pub start: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneLayer {
    pub layer_id: u64,
    pub node_id: u64,
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
pub struct SceneBlock {
    pub node_id: u64,
    pub layer_id: u64,
    pub order: u32,
    pub bounds: Rect,
    pub transform: SceneTransform,
    pub clip: Option<SceneClip>,
    pub commands: CommandRange,
    pub command_count: usize,
    pub command_signature: u64,
    pub resource_keys: Vec<SceneResourceKey>,
    staged_commands: Option<Vec<DrawCommand>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenePatch {
    pub size: Size,
    pub commands: Vec<DrawCommand>,
    pub base_layer_count: usize,
    pub base_block_count: usize,
    pub layer_upserts: Vec<SceneLayer>,
    pub layer_reorders: Vec<SceneLayerOrder>,
    pub layer_removes: Vec<u64>,
    pub upserts: Vec<SceneBlock>,
    pub reorders: Vec<SceneBlockOrder>,
    pub removes: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneLayerOrder {
    pub layer_id: u64,
    pub order: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneBlockOrder {
    pub node_id: u64,
    pub order: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SceneSubmit {
    Full(Scene),
    Patch { patch: ScenePatch, current: Scene },
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
            commands: Vec::new(),
            layers: vec![SceneLayer::root(size)],
            blocks: Vec::new(),
        }
    }

    #[must_use]
    pub fn from_blocks(size: Size, clear_color: Option<Color>, blocks: Vec<SceneBlock>) -> Self {
        Self::from_layers_and_blocks(size, clear_color, vec![SceneLayer::root(size)], blocks)
    }

    #[must_use]
    pub fn from_layers_and_blocks(
        size: Size,
        clear_color: Option<Color>,
        layers: Vec<SceneLayer>,
        blocks: Vec<SceneBlock>,
    ) -> Self {
        let (commands, blocks) = Self::compact_blocks(blocks);
        Self::from_layers_and_blocks_with_commands(size, clear_color, layers, commands, blocks)
    }

    #[must_use]
    pub fn from_layers_and_blocks_with_commands(
        size: Size,
        clear_color: Option<Color>,
        mut layers: Vec<SceneLayer>,
        commands: Vec<DrawCommand>,
        blocks: Vec<SceneBlock>,
    ) -> Self {
        if !layers
            .iter()
            .any(|layer| layer.layer_id == Self::ROOT_LAYER_ID)
        {
            layers.push(SceneLayer::root(size));
        }
        layers.sort_by_key(|layer| layer.order);
        Self {
            size,
            clear_color,
            commands,
            layers,
            blocks,
        }
    }

    pub fn push(&mut self, command: DrawCommand) {
        self.layers = vec![SceneLayer::root(self.size)];
        let start = self.commands.len();
        let resource_keys = command.resource_key().into_iter().collect();
        let signature = command_signature(std::slice::from_ref(&command));
        self.commands.push(command);
        self.blocks.push(SceneBlock::with_range(
            u64::MAX - self.blocks.len() as u64,
            Self::ROOT_LAYER_ID,
            self.blocks.len() as u32,
            Rect::new(0.0, 0.0, self.size.width, self.size.height),
            Transform2D::identity(),
            None,
            CommandRange { start, len: 1 },
            1,
            signature,
            resource_keys,
        ));
    }

    #[must_use]
    pub fn iter_commands(&self) -> impl Iterator<Item = &DrawCommand> {
        self.commands.iter()
    }

    #[must_use]
    pub fn command_count(&self) -> usize {
        self.commands.len()
            + usize::from(self.clear_color.is_some() && self.clear_command().is_none())
    }

    #[must_use]
    pub fn clear_command(&self) -> Option<Color> {
        self.iter_commands().find_map(|command| match command {
            DrawCommand::Clear(color) => Some(*color),
            _ => None,
        })
    }

    #[must_use]
    pub fn resource_keys(&self) -> Vec<SceneResourceKey> {
        self.blocks
            .iter()
            .flat_map(|block| block.resource_keys.iter().copied())
            .collect()
    }

    #[must_use]
    pub fn commands_for_block<'a>(&'a self, block: &'a SceneBlock) -> &'a [DrawCommand] {
        &self.commands[block.commands.start..block.commands.start + block.commands.len]
    }

    #[must_use]
    pub fn apply_patch(&self, patch: &ScenePatch) -> Self {
        let mut layers: Vec<SceneLayer> = self
            .layers
            .iter()
            .filter(|layer| !patch.layer_removes.contains(&layer.layer_id))
            .cloned()
            .collect();
        for upsert in &patch.layer_upserts {
            if let Some(existing) = layers
                .iter_mut()
                .find(|layer| layer.layer_id == upsert.layer_id)
            {
                *existing = upsert.clone();
            } else {
                layers.push(upsert.clone());
            }
        }
        for reorder in &patch.layer_reorders {
            if let Some(existing) = layers
                .iter_mut()
                .find(|layer| layer.layer_id == reorder.layer_id)
            {
                existing.order = reorder.order;
            }
        }
        layers.sort_by_key(|layer| layer.order);

        let mut blocks: Vec<(SceneBlock, bool)> = self
            .blocks
            .iter()
            .filter(|block| !patch.removes.contains(&block.node_id))
            .cloned()
            .map(|block| (block, false))
            .collect();

        for upsert in &patch.upserts {
            if let Some(existing) = blocks
                .iter_mut()
                .find(|(block, _)| block.node_id == upsert.node_id)
            {
                *existing = (upsert.clone(), true);
            } else {
                blocks.push((upsert.clone(), true));
            }
        }
        for reorder in &patch.reorders {
            if let Some(existing) = blocks
                .iter_mut()
                .find(|(block, _)| block.node_id == reorder.node_id)
            {
                existing.0.order = reorder.order;
            }
        }

        blocks.sort_by_key(|(block, _)| block.order);
        let mut commands = Vec::new();
        let rebuilt_blocks = blocks
            .into_iter()
            .map(|(block, from_patch)| {
                let block_commands = if from_patch {
                    patch.commands_for_block(&block).to_vec()
                } else {
                    self.commands_for_block(&block).to_vec()
                };
                let start = commands.len();
                commands.extend_from_slice(&block_commands);
                let mut block = block;
                block = block.with_normalized_commands(CommandRange {
                    start,
                    len: block_commands.len(),
                });
                block
            })
            .collect();
        Self::from_layers_and_blocks_with_commands(patch.size, self.clear_color, layers, commands, rebuilt_blocks)
    }

    #[must_use]
    pub fn dirty_bounds_for_nodes(&self, node_ids: &[u64]) -> Option<Rect> {
        self.blocks
            .iter()
            .filter(|block| node_ids.contains(&block.node_id))
            .filter_map(|block| self.effective_bounds_for_block(block))
            .reduce(|acc, bounds| acc.union(&bounds))
    }

    #[must_use]
    pub fn dirty_bounds_for_layers(&self, layer_ids: &[u64]) -> Option<Rect> {
        self.layers
            .iter()
            .filter(|layer| layer_ids.contains(&layer.layer_id))
            .filter_map(|layer| self.effective_bounds_for_layer(layer.layer_id))
            .reduce(|acc, bounds| acc.union(&bounds))
    }

    #[must_use]
    pub fn effective_bounds_for_layer(&self, layer_id: u64) -> Option<Rect> {
        let layer = self
            .layers
            .iter()
            .find(|layer| layer.layer_id == layer_id)?;
        self.effective_clip_bounds_for_layer(layer_id)
            .map_or(Some(layer.bounds), |clip_bounds| {
                rect_intersection(layer.bounds, clip_bounds)
            })
    }

    #[must_use]
    pub fn effective_bounds_for_block(&self, block: &SceneBlock) -> Option<Rect> {
        let clipped = self
            .effective_clip_bounds_for_layer(block.layer_id)
            .map_or(Some(block.bounds), |clip_bounds| {
                rect_intersection(block.bounds, clip_bounds)
            })?;
        block.clip.map_or(Some(clipped), |clip| {
            let layer_transform = self.layer_world_transform(block.layer_id)?;
            rect_intersection(clipped, layer_transform.map_rect(scene_clip_bounds(clip)))
        })
    }

    #[must_use]
    pub fn effective_clip_bounds_for_layer(&self, layer_id: u64) -> Option<Rect> {
        let mut current_layer_id = Some(layer_id);
        let mut clip_bounds = None;
        while let Some(id) = current_layer_id {
            let layer = self.layers.iter().find(|layer| layer.layer_id == id)?;
            if let Some(clip) = layer.clip {
                let world_bounds = self
                    .layer_world_transform(id)?
                    .map_rect(scene_clip_bounds(clip));
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
        let layer = self
            .layers
            .iter()
            .find(|layer| layer.layer_id == layer_id)?;
        layer.parent_layer_id.map_or_else(
            || Some(layer.transform),
            |parent_id| {
                self.layer_world_transform(parent_id)
                    .map(|transform| transform.then(layer.transform))
            },
        )
    }

    #[must_use]
    pub fn compact_blocks(blocks: Vec<SceneBlock>) -> (Vec<DrawCommand>, Vec<SceneBlock>) {
        let mut commands = Vec::new();
        let normalized = blocks
            .into_iter()
            .map(|block| {
                let block_commands = block
                    .staged_commands
                    .clone()
                    .unwrap_or_default();
                let start = commands.len();
                commands.extend_from_slice(&block_commands);
                block.with_normalized_commands(CommandRange {
                    start,
                    len: block_commands.len(),
                })
            })
            .collect();
        (commands, normalized)
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

fn command_signature(commands: &[DrawCommand]) -> u64 {
    let mut hasher = DefaultHasher::new();
    commands.len().hash(&mut hasher);
    for command in commands {
        match command {
            DrawCommand::Fill { shape, brush } => {
                1u8.hash(&mut hasher);
                hash_shape(shape, &mut hasher);
                hash_brush(brush, &mut hasher);
            }
            DrawCommand::Stroke {
                shape,
                stroke,
            } => {
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

impl SceneLayer {
    #[must_use]
    pub fn root(size: Size) -> Self {
        Self {
            layer_id: Scene::ROOT_LAYER_ID,
            node_id: Scene::ROOT_LAYER_ID,
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
        node_id: u64,
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
            node_id,
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

impl SceneBlock {
    #[must_use]
    pub fn new(
        node_id: u64,
        layer_id: u64,
        order: u32,
        bounds: Rect,
        transform: SceneTransform,
        clip: Option<SceneClip>,
        commands: Vec<DrawCommand>,
    ) -> Self {
        Self::from_commands(node_id, layer_id, order, bounds, transform, clip, commands)
    }

    #[must_use]
    pub fn with_range(
        node_id: u64,
        layer_id: u64,
        order: u32,
        bounds: Rect,
        transform: SceneTransform,
        clip: Option<SceneClip>,
        commands: CommandRange,
        command_count: usize,
        command_signature: u64,
        resource_keys: Vec<SceneResourceKey>,
    ) -> Self {
        Self {
            node_id,
            layer_id,
            order,
            bounds,
            transform,
            clip,
            commands,
            command_count,
            command_signature,
            resource_keys,
            staged_commands: None,
        }
    }

    #[must_use]
    pub fn from_commands(
        node_id: u64,
        layer_id: u64,
        order: u32,
        bounds: Rect,
        transform: SceneTransform,
        clip: Option<SceneClip>,
        commands: Vec<DrawCommand>,
    ) -> Self {
        let command_count = commands.len();
        let command_signature = command_signature(&commands);
        let resource_keys = commands.iter().filter_map(DrawCommand::resource_key).collect();
        Self {
            node_id,
            layer_id,
            order,
            bounds,
            transform,
            clip,
            commands: CommandRange {
                start: 0,
                len: command_count,
            },
            command_count,
            command_signature,
            resource_keys,
            staged_commands: Some(commands),
        }
    }

    #[must_use]
    pub fn with_normalized_commands(mut self, range: CommandRange) -> Self {
        self.commands = range;
        self.command_count = range.len;
        self.staged_commands = None;
        self
    }
}

impl ScenePatch {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layer_upserts.is_empty()
            && self.layer_reorders.is_empty()
            && self.layer_removes.is_empty()
            && self.upserts.is_empty()
            && self.reorders.is_empty()
            && self.removes.is_empty()
    }

    #[must_use]
    pub fn dirty_bounds(&self, previous: Option<&Scene>) -> Option<Rect> {
        let upsert_bounds = self
            .upserts
            .iter()
            .map(|block| block.bounds)
            .reduce(|acc, bounds| acc.union(&bounds));
        let affected_layer_bounds = previous.and_then(|scene| {
            let mut layer_ids: Vec<u64> = self
                .upserts
                .iter()
                .map(|block| block.layer_id)
                .filter(|layer_id| *layer_id != Scene::ROOT_LAYER_ID)
                .collect();
            layer_ids.extend(
                scene
                    .blocks
                    .iter()
                    .filter(|block| self.removes.contains(&block.node_id))
                    .map(|block| block.layer_id)
                    .filter(|layer_id| *layer_id != Scene::ROOT_LAYER_ID),
            );
            layer_ids.sort_unstable();
            layer_ids.dedup();
            scene.dirty_bounds_for_layers(&layer_ids)
        });
        let previous_upsert_bounds = previous.and_then(|scene| {
            let node_ids: Vec<u64> = self.upserts.iter().map(|block| block.node_id).collect();
            scene.dirty_bounds_for_nodes(&node_ids)
        });
        let remove_bounds = previous.and_then(|scene| scene.dirty_bounds_for_nodes(&self.removes));
        let reorder_bounds = previous.and_then(|scene| {
            let node_ids: Vec<u64> = self
                .reorders
                .iter()
                .map(|reorder| reorder.node_id)
                .collect();
            scene.dirty_bounds_for_nodes(&node_ids)
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
            let layer_ids: Vec<u64> = self
                .layer_upserts
                .iter()
                .map(|layer| layer.layer_id)
                .collect();
            scene.dirty_bounds_for_layers(&layer_ids)
        });
        let layer_remove_bounds =
            previous.and_then(|scene| scene.dirty_bounds_for_layers(&self.layer_removes));
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
    pub fn commands_for_block<'a>(&'a self, block: &'a SceneBlock) -> &'a [DrawCommand] {
        if let Some(commands) = block.staged_commands.as_ref() {
            return commands.as_slice();
        }
        &self.commands[block.commands.start..block.commands.start + block.commands.len]
    }
}

impl SceneSubmit {
    #[must_use]
    pub fn snapshot(&self, previous: Option<&Scene>) -> Option<Scene> {
        match self {
            Self::Full(scene) => Some(scene.clone()),
            Self::Patch { patch, current } => previous.map_or_else(
                || Some(current.clone()),
                |scene| Some(scene.apply_patch(patch)),
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

#[cfg(test)]
mod tests {
    use super::{
        Brush, DrawCommand, Scene, SceneBlendMode, SceneBlock, SceneBlockOrder, SceneEffect,
        SceneLayer, SceneLayerOrder, ScenePatch, SceneSubmit, Shape,
    };
    use zeno_core::{Color, Rect, Size, Transform2D};

    #[test]
    fn dirty_bounds_merge_upserts_and_removed_blocks() {
        let previous = Scene::from_blocks(
            Size::new(200.0, 200.0),
            None,
            vec![
                SceneBlock::new(
                    1,
                    Scene::ROOT_LAYER_ID,
                    0,
                    Rect::new(0.0, 0.0, 20.0, 20.0),
                    Transform2D::identity(),
                    None,
                    vec![DrawCommand::Fill {
                        shape: Shape::Rect(Rect::new(0.0, 0.0, 20.0, 20.0)),
                        brush: Brush::Solid(Color::WHITE),
                    }],
                ),
                SceneBlock::new(
                    2,
                    Scene::ROOT_LAYER_ID,
                    1,
                    Rect::new(80.0, 80.0, 40.0, 40.0),
                    Transform2D::identity(),
                    None,
                    vec![DrawCommand::Fill {
                        shape: Shape::Rect(Rect::new(80.0, 80.0, 40.0, 40.0)),
                        brush: Brush::Solid(Color::WHITE),
                    }],
                ),
            ],
        );
        let patch = ScenePatch {
            size: previous.size,
            commands: Vec::new(),
            base_layer_count: previous.layers.len(),
            base_block_count: previous.blocks.len(),
            layer_upserts: Vec::new(),
            layer_reorders: Vec::new(),
            layer_removes: Vec::new(),
            upserts: vec![SceneBlock::new(
                3,
                Scene::ROOT_LAYER_ID,
                2,
                Rect::new(40.0, 40.0, 10.0, 10.0),
                Transform2D::identity(),
                None,
                vec![DrawCommand::Fill {
                    shape: Shape::Rect(Rect::new(40.0, 40.0, 10.0, 10.0)),
                    brush: Brush::Solid(Color::WHITE),
                }],
            )],
            reorders: Vec::new(),
            removes: vec![2],
        };

        assert_eq!(
            patch.dirty_bounds(Some(&previous)),
            Some(Rect::new(40.0, 40.0, 80.0, 80.0))
        );
    }

    #[test]
    fn patch_snapshot_without_previous_uses_current_scene() {
        let current = Scene::from_blocks(
            Size::new(200.0, 200.0),
            None,
            vec![SceneBlock::new(
                1,
                Scene::ROOT_LAYER_ID,
                0,
                Rect::new(10.0, 10.0, 30.0, 30.0),
                Transform2D::identity(),
                None,
                vec![DrawCommand::Fill {
                    shape: Shape::Rect(Rect::new(10.0, 10.0, 30.0, 30.0)),
                    brush: Brush::Solid(Color::WHITE),
                }],
            )],
        );
        let submit = SceneSubmit::Patch {
            patch: ScenePatch {
                size: current.size,
                commands: Vec::new(),
                base_layer_count: 0,
                base_block_count: 0,
                layer_upserts: current.layers.clone(),
                layer_reorders: Vec::new(),
                layer_removes: Vec::new(),
                upserts: current.blocks.clone(),
                reorders: Vec::new(),
                removes: Vec::new(),
            },
            current: current.clone(),
        };

        assert_eq!(submit.snapshot(None), Some(current));
    }

    #[test]
    fn apply_patch_supports_order_only_updates() {
        let previous = Scene::from_layers_and_blocks(
            Size::new(200.0, 200.0),
            None,
            vec![
                SceneLayer::root(Size::new(200.0, 200.0)),
                SceneLayer::new(
                    10,
                    10,
                    Some(Scene::ROOT_LAYER_ID),
                    1,
                    Rect::new(0.0, 0.0, 40.0, 40.0),
                    Rect::new(0.0, 0.0, 40.0, 40.0),
                    Transform2D::identity(),
                    None,
                    1.0,
                    SceneBlendMode::Normal,
                    Vec::new(),
                    false,
                ),
                SceneLayer::new(
                    20,
                    20,
                    Some(Scene::ROOT_LAYER_ID),
                    3,
                    Rect::new(50.0, 0.0, 40.0, 40.0),
                    Rect::new(50.0, 0.0, 40.0, 40.0),
                    Transform2D::identity(),
                    None,
                    1.0,
                    SceneBlendMode::Normal,
                    Vec::new(),
                    false,
                ),
            ],
            vec![
                SceneBlock::new(
                    1,
                    Scene::ROOT_LAYER_ID,
                    2,
                    Rect::new(0.0, 0.0, 20.0, 20.0),
                    Transform2D::identity(),
                    None,
                    vec![DrawCommand::Fill {
                        shape: Shape::Rect(Rect::new(0.0, 0.0, 20.0, 20.0)),
                        brush: Brush::Solid(Color::WHITE),
                    }],
                ),
                SceneBlock::new(
                    2,
                    Scene::ROOT_LAYER_ID,
                    4,
                    Rect::new(30.0, 0.0, 20.0, 20.0),
                    Transform2D::identity(),
                    None,
                    vec![DrawCommand::Fill {
                        shape: Shape::Rect(Rect::new(30.0, 0.0, 20.0, 20.0)),
                        brush: Brush::Solid(Color::BLACK),
                    }],
                ),
            ],
        );
        let patch = ScenePatch {
            size: previous.size,
            commands: Vec::new(),
            base_layer_count: previous.layers.len(),
            base_block_count: previous.blocks.len(),
            layer_upserts: Vec::new(),
            layer_reorders: vec![
                SceneLayerOrder {
                    layer_id: 20,
                    order: 1,
                },
                SceneLayerOrder {
                    layer_id: 10,
                    order: 3,
                },
            ],
            layer_removes: Vec::new(),
            upserts: Vec::new(),
            reorders: vec![
                SceneBlockOrder {
                    node_id: 2,
                    order: 2,
                },
                SceneBlockOrder {
                    node_id: 1,
                    order: 4,
                },
            ],
            removes: Vec::new(),
        };

        let current = previous.apply_patch(&patch);

        assert_eq!(current.layers[1].layer_id, 20);
        assert_eq!(current.layers[2].layer_id, 10);
        assert_eq!(current.blocks[0].node_id, 2);
        assert_eq!(current.blocks[1].node_id, 1);
        assert_eq!(
            patch.dirty_bounds(Some(&previous)),
            Some(Rect::new(0.0, 0.0, 90.0, 40.0))
        );
    }

    #[test]
    fn dirty_bounds_include_previous_bounds_for_moved_upserts() {
        let previous = Scene::from_blocks(
            Size::new(200.0, 200.0),
            None,
            vec![SceneBlock::new(
                1,
                Scene::ROOT_LAYER_ID,
                0,
                Rect::new(0.0, 0.0, 20.0, 20.0),
                Transform2D::identity(),
                None,
                vec![DrawCommand::Fill {
                    shape: Shape::Rect(Rect::new(0.0, 0.0, 20.0, 20.0)),
                    brush: Brush::Solid(Color::WHITE),
                }],
            )],
        );
        let patch = ScenePatch {
            size: previous.size,
            commands: Vec::new(),
            base_layer_count: previous.layers.len(),
            base_block_count: previous.blocks.len(),
            layer_upserts: Vec::new(),
            layer_reorders: Vec::new(),
            layer_removes: Vec::new(),
            upserts: vec![SceneBlock::new(
                1,
                Scene::ROOT_LAYER_ID,
                0,
                Rect::new(40.0, 0.0, 20.0, 20.0),
                Transform2D::translation(40.0, 0.0),
                None,
                vec![DrawCommand::Fill {
                    shape: Shape::Rect(Rect::new(0.0, 0.0, 20.0, 20.0)),
                    brush: Brush::Solid(Color::WHITE),
                }],
            )],
            reorders: Vec::new(),
            removes: Vec::new(),
        };

        assert_eq!(
            patch.dirty_bounds(Some(&previous)),
            Some(Rect::new(0.0, 0.0, 60.0, 20.0))
        );
    }

    #[test]
    fn dirty_bounds_include_layer_changes_without_block_changes() {
        let previous = Scene::from_layers_and_blocks(
            Size::new(200.0, 200.0),
            None,
            vec![
                SceneLayer::root(Size::new(200.0, 200.0)),
                SceneLayer::new(
                    10,
                    10,
                    Some(Scene::ROOT_LAYER_ID),
                    1,
                    Rect::new(0.0, 0.0, 40.0, 40.0),
                    Rect::new(20.0, 20.0, 40.0, 40.0),
                    Transform2D::translation(20.0, 20.0),
                    None,
                    1.0,
                    SceneBlendMode::Normal,
                    Vec::new(),
                    false,
                ),
            ],
            Vec::new(),
        );
        let patch = ScenePatch {
            size: previous.size,
            commands: Vec::new(),
            base_layer_count: previous.layers.len(),
            base_block_count: previous.blocks.len(),
            layer_upserts: vec![SceneLayer::new(
                10,
                10,
                Some(Scene::ROOT_LAYER_ID),
                1,
                Rect::new(0.0, 0.0, 40.0, 40.0),
                Rect::new(20.0, 20.0, 40.0, 40.0),
                Transform2D::translation(20.0, 20.0),
                None,
                0.5,
                SceneBlendMode::Normal,
                Vec::new(),
                true,
            )],
            layer_reorders: Vec::new(),
            layer_removes: Vec::new(),
            upserts: Vec::new(),
            reorders: Vec::new(),
            removes: Vec::new(),
        };

        assert_eq!(
            patch.dirty_bounds(Some(&previous)),
            Some(Rect::new(20.0, 20.0, 40.0, 40.0))
        );
    }

    #[test]
    fn dirty_bounds_include_removed_layers() {
        let previous = Scene::from_layers_and_blocks(
            Size::new(200.0, 200.0),
            None,
            vec![
                SceneLayer::root(Size::new(200.0, 200.0)),
                SceneLayer::new(
                    10,
                    10,
                    Some(Scene::ROOT_LAYER_ID),
                    1,
                    Rect::new(0.0, 0.0, 40.0, 40.0),
                    Rect::new(20.0, 20.0, 40.0, 40.0),
                    Transform2D::translation(20.0, 20.0),
                    None,
                    1.0,
                    SceneBlendMode::Normal,
                    Vec::new(),
                    false,
                ),
            ],
            Vec::new(),
        );
        let patch = ScenePatch {
            size: previous.size,
            commands: Vec::new(),
            base_layer_count: previous.layers.len(),
            base_block_count: previous.blocks.len(),
            layer_upserts: Vec::new(),
            layer_reorders: Vec::new(),
            layer_removes: vec![10],
            upserts: Vec::new(),
            reorders: Vec::new(),
            removes: Vec::new(),
        };

        assert_eq!(
            patch.dirty_bounds(Some(&previous)),
            Some(Rect::new(20.0, 20.0, 40.0, 40.0))
        );
    }

    #[test]
    fn dirty_bounds_expand_block_updates_to_effect_layer_bounds() {
        let previous = Scene::from_layers_and_blocks(
            Size::new(200.0, 200.0),
            None,
            vec![
                SceneLayer::root(Size::new(200.0, 200.0)),
                SceneLayer::new(
                    10,
                    10,
                    Some(Scene::ROOT_LAYER_ID),
                    1,
                    Rect::new(0.0, 0.0, 40.0, 40.0),
                    Rect::new(-12.0, -12.0, 64.0, 64.0),
                    Transform2D::identity(),
                    None,
                    1.0,
                    SceneBlendMode::Normal,
                    vec![SceneEffect::Blur { sigma: 4.0 }],
                    true,
                ),
            ],
            vec![SceneBlock::new(
                10,
                10,
                2,
                Rect::new(0.0, 0.0, 40.0, 40.0),
                Transform2D::identity(),
                None,
                vec![DrawCommand::Fill {
                    shape: Shape::Rect(Rect::new(0.0, 0.0, 40.0, 40.0)),
                    brush: Brush::Solid(Color::WHITE),
                }],
            )],
        );
        let patch = ScenePatch {
            size: previous.size,
            commands: Vec::new(),
            base_layer_count: previous.layers.len(),
            base_block_count: previous.blocks.len(),
            layer_upserts: Vec::new(),
            layer_reorders: Vec::new(),
            layer_removes: Vec::new(),
            upserts: vec![SceneBlock::new(
                10,
                10,
                2,
                Rect::new(0.0, 0.0, 40.0, 40.0),
                Transform2D::identity(),
                None,
                vec![DrawCommand::Fill {
                    shape: Shape::Rect(Rect::new(0.0, 0.0, 40.0, 40.0)),
                    brush: Brush::Solid(Color::BLACK),
                }],
            )],
            reorders: Vec::new(),
            removes: Vec::new(),
        };

        assert_eq!(
            patch.dirty_bounds(Some(&previous)),
            Some(Rect::new(-12.0, -12.0, 64.0, 64.0))
        );
    }

    #[test]
    fn effective_bounds_respect_ancestor_clip_chain() {
        let scene = Scene::from_layers_and_blocks(
            Size::new(200.0, 200.0),
            None,
            vec![
                SceneLayer::root(Size::new(200.0, 200.0)),
                SceneLayer::new(
                    10,
                    10,
                    Some(Scene::ROOT_LAYER_ID),
                    1,
                    Rect::new(0.0, 0.0, 120.0, 120.0),
                    Rect::new(20.0, 20.0, 120.0, 120.0),
                    Transform2D::translation(20.0, 20.0),
                    Some(super::SceneClip::Rect(Rect::new(0.0, 0.0, 60.0, 60.0))),
                    1.0,
                    SceneBlendMode::Normal,
                    Vec::new(),
                    false,
                ),
                SceneLayer::new(
                    20,
                    20,
                    Some(10),
                    2,
                    Rect::new(0.0, 0.0, 80.0, 80.0),
                    Rect::new(40.0, 40.0, 80.0, 80.0),
                    Transform2D::translation(20.0, 20.0),
                    None,
                    1.0,
                    SceneBlendMode::Normal,
                    Vec::new(),
                    false,
                ),
            ],
            vec![SceneBlock::new(
                30,
                20,
                3,
                Rect::new(40.0, 40.0, 80.0, 80.0),
                Transform2D::identity(),
                None,
                vec![DrawCommand::Fill {
                    shape: Shape::Rect(Rect::new(0.0, 0.0, 80.0, 80.0)),
                    brush: Brush::Solid(Color::WHITE),
                }],
            )],
        );

        assert_eq!(
            scene.effective_bounds_for_layer(20),
            Some(Rect::new(40.0, 40.0, 40.0, 40.0))
        );
        assert_eq!(
            scene.dirty_bounds_for_nodes(&[30]),
            Some(Rect::new(40.0, 40.0, 40.0, 40.0))
        );
    }
}
