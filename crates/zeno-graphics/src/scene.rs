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
    pub commands: Vec<DrawCommand>,
    pub resource_keys: Vec<SceneResourceKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenePatch {
    pub size: Size,
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
        mut layers: Vec<SceneLayer>,
        blocks: Vec<SceneBlock>,
    ) -> Self {
        if !layers
            .iter()
            .any(|layer| layer.layer_id == Self::ROOT_LAYER_ID)
        {
            layers.push(SceneLayer::root(size));
        }
        layers.sort_by_key(|layer| layer.order);
        let commands = blocks
            .iter()
            .flat_map(|block| block.commands.iter().cloned())
            .collect();
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
        self.blocks.clear();
        self.commands.push(command);
    }

    #[must_use]
    pub fn resource_keys(&self) -> Vec<SceneResourceKey> {
        self.commands
            .iter()
            .filter_map(DrawCommand::resource_key)
            .collect()
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

        let mut blocks: Vec<SceneBlock> = self
            .blocks
            .iter()
            .filter(|block| !patch.removes.contains(&block.node_id))
            .cloned()
            .collect();

        for upsert in &patch.upserts {
            if let Some(existing) = blocks
                .iter_mut()
                .find(|block| block.node_id == upsert.node_id)
            {
                *existing = upsert.clone();
            } else {
                blocks.push(upsert.clone());
            }
        }
        for reorder in &patch.reorders {
            if let Some(existing) = blocks
                .iter_mut()
                .find(|block| block.node_id == reorder.node_id)
            {
                existing.order = reorder.order;
            }
        }

        blocks.sort_by_key(|block| block.order);
        Self::from_layers_and_blocks(patch.size, self.clear_color, layers, blocks)
    }

    #[must_use]
    pub fn dirty_bounds_for_nodes(&self, node_ids: &[u64]) -> Option<Rect> {
        self.blocks
            .iter()
            .filter(|block| node_ids.contains(&block.node_id))
            .map(|block| block.bounds)
            .reduce(|acc, bounds| acc.union(&bounds))
    }

    #[must_use]
    pub fn dirty_bounds_for_layers(&self, layer_ids: &[u64]) -> Option<Rect> {
        self.layers
            .iter()
            .filter(|layer| layer_ids.contains(&layer.layer_id))
            .map(|layer| layer.bounds)
            .reduce(|acc, bounds| acc.union(&bounds))
    }
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
        let resource_keys = commands
            .iter()
            .filter_map(DrawCommand::resource_key)
            .collect();
        Self {
            node_id,
            layer_id,
            order,
            bounds,
            transform,
            clip,
            commands,
            resource_keys,
        }
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
}
