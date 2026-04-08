use zeno_core::{Color, Point, Rect, Size};
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
    Text { position: Point, layout: TextLayout, color: Color },
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
    pub commands: Vec<DrawCommand>,
    pub blocks: Vec<SceneBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneBlock {
    pub node_id: u64,
    pub order: u32,
    pub bounds: Rect,
    pub commands: Vec<DrawCommand>,
    pub resource_keys: Vec<SceneResourceKey>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenePatch {
    pub size: Size,
    pub base_block_count: usize,
    pub upserts: Vec<SceneBlock>,
    pub removes: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SceneSubmit {
    Full(Scene),
    Patch { patch: ScenePatch, current: Scene },
}

impl Scene {
    #[must_use]
    pub fn new(size: Size) -> Self {
        Self {
            size,
            commands: Vec::new(),
            blocks: Vec::new(),
        }
    }

    #[must_use]
    pub fn from_blocks(size: Size, blocks: Vec<SceneBlock>) -> Self {
        let commands = blocks
            .iter()
            .flat_map(|block| block.commands.iter().cloned())
            .collect();
        Self {
            size,
            commands,
            blocks,
        }
    }

    pub fn push(&mut self, command: DrawCommand) {
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
        let mut blocks: Vec<SceneBlock> = self
            .blocks
            .iter()
            .filter(|block| !patch.removes.contains(&block.node_id))
            .cloned()
            .collect();

        for upsert in &patch.upserts {
            if let Some(existing) = blocks.iter_mut().find(|block| block.node_id == upsert.node_id) {
                *existing = upsert.clone();
            } else {
                blocks.push(upsert.clone());
            }
        }

        blocks.sort_by_key(|block| block.order);
        Self::from_blocks(patch.size, blocks)
    }
}

impl SceneBlock {
    #[must_use]
    pub fn new(node_id: u64, order: u32, bounds: Rect, commands: Vec<DrawCommand>) -> Self {
        let resource_keys = commands.iter().filter_map(DrawCommand::resource_key).collect();
        Self {
            node_id,
            order,
            bounds,
            commands,
            resource_keys,
        }
    }
}

impl ScenePatch {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.upserts.is_empty() && self.removes.is_empty()
    }
}

impl SceneSubmit {
    #[must_use]
    pub fn snapshot(&self, previous: Option<&Scene>) -> Option<Scene> {
        match self {
            Self::Full(scene) => Some(scene.clone()),
            Self::Patch { patch, current } => {
                previous.map_or_else(|| Some(current.clone()), |scene| Some(scene.apply_patch(patch)))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanvasOp {
    Save,
    Restore,
    Translate { x: f32, y: f32 },
}
