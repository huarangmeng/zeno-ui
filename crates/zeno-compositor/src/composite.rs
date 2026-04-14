use zeno_core::{Color, Rect};

use crate::tile::{RasterBatch, TileContentHandle, TileGrid, TileId, TilePlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompositorLayerId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorBlendMode {
    Normal,
    Multiply,
    Screen,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompositorEffect {
    Blur { sigma: f32 },
    DropShadow {
        dx: f32,
        dy: f32,
        blur: f32,
        color: Color,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositorLayer {
    pub layer_id: CompositorLayerId,
    pub parent: Option<CompositorLayerId>,
    pub stacking_context_index: Option<usize>,
    pub opacity: f32,
    pub blend_mode: CompositorBlendMode,
    pub effects: Vec<CompositorEffect>,
    pub needs_offscreen: bool,
    pub bounds: Rect,
    pub effect_bounds: Rect,
    pub effect_padding: f32,
    pub item_count: usize,
    pub tile_ids: Vec<TileId>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CompositorLayerTree {
    pub layers: Vec<CompositorLayer>,
}

impl CompositorLayerTree {
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    #[must_use]
    pub fn offscreen_layer_count(&self) -> usize {
        self.layers
            .iter()
            .filter(|layer| layer.needs_offscreen)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositeLayerPass {
    pub layer_id: CompositorLayerId,
    pub tiles: Vec<CompositeTileRef>,
    pub needs_offscreen: bool,
    pub opacity: f32,
    pub blend_mode: CompositorBlendMode,
    pub effects: Vec<CompositorEffect>,
    pub bounds: Rect,
    pub effect_bounds: Rect,
    pub effect_padding: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompositeTileRef {
    pub tile_id: TileId,
    pub content_handle: TileContentHandle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositePass {
    pub steps: Vec<CompositeLayerPass>,
    pub full_present: bool,
}

impl CompositePass {
    #[must_use]
    pub fn tile_count(&self) -> usize {
        self.steps.iter().map(|step| step.tiles.len()).sum()
    }

    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.steps.len()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CompositeExecutionStats {
    pub executed_layer_count: usize,
    pub executed_tile_count: usize,
    pub offscreen_step_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompositeTileJob {
    pub layer_id: CompositorLayerId,
    pub content_handle: TileContentHandle,
    pub rect: Rect,
    pub opacity: f32,
    pub needs_offscreen: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositeLayerJob {
    pub layer_id: CompositorLayerId,
    pub opacity: f32,
    pub blend_mode: CompositorBlendMode,
    pub effects: Vec<CompositorEffect>,
    pub needs_offscreen: bool,
    pub bounds: Rect,
    pub effect_bounds: Rect,
    pub effect_padding: f32,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CompositeExecutionPlan {
    pub layer_jobs: Vec<CompositeLayerJob>,
    pub jobs: Vec<CompositeTileJob>,
    pub stats: CompositeExecutionStats,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CompositeExecutor;

impl CompositeExecutor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn execute(&self, pass: &CompositePass) -> CompositeExecutionStats {
        CompositeExecutionStats {
            executed_layer_count: pass.layer_count(),
            executed_tile_count: pass.tile_count(),
            offscreen_step_count: pass
                .steps
                .iter()
                .filter(|step| step.needs_offscreen)
                .count(),
        }
    }

    #[must_use]
    pub fn plan(&self, pass: &CompositePass, grid: TileGrid) -> CompositeExecutionPlan {
        let layer_jobs = pass
            .steps
            .iter()
            .map(|step| CompositeLayerJob {
                layer_id: step.layer_id,
                opacity: step.opacity,
                blend_mode: step.blend_mode,
                effects: step.effects.clone(),
                needs_offscreen: step.needs_offscreen,
                bounds: step.bounds,
                effect_bounds: step.effect_bounds,
                effect_padding: step.effect_padding,
            })
            .collect::<Vec<_>>();
        let jobs = pass
            .steps
            .iter()
            .flat_map(|step| {
                step.tiles.iter().filter_map(|tile| {
                    grid.tile_rect(tile.tile_id).map(|rect| CompositeTileJob {
                        layer_id: step.layer_id,
                        content_handle: tile.content_handle,
                        rect,
                        opacity: step.opacity,
                        needs_offscreen: step.needs_offscreen,
                    })
                })
            })
            .collect::<Vec<_>>();
        CompositeExecutionPlan {
            layer_jobs,
            stats: self.execute(pass),
            jobs,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositorSubmission {
    pub tile_plan: TilePlan,
    pub raster_batch: RasterBatch,
    pub composite_pass: CompositePass,
    pub layer_tree: CompositorLayerTree,
}
