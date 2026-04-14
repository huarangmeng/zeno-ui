mod composite;
mod damage;
mod frame;
mod scheduler;
mod tile;

pub use composite::{
    CompositeExecutionPlan, CompositeExecutionStats, CompositeExecutor, CompositeLayerJob,
    CompositeLayerPass, CompositePass, CompositeTileJob, CompositeTileRef, CompositorBlendMode,
    CompositorEffect, CompositorLayer, CompositorLayerId, CompositorLayerTree,
    CompositorSubmission,
};
pub use damage::{DamageRegion, DamageTracker};
pub use frame::{CompositorFrame, CompositorFrameStats};
pub use scheduler::{
    CompositorScheduler, CompositorSchedulerStats, CompositorService, CompositorServiceStats,
    CompositorTask, CompositorWorker, CompositorWorkerOutput, CompositorWorkerStats,
    ScheduledCompositorFrame, ThreadedCompositorWorker,
};
pub use tile::{
    RasterBatch, RasterTile, TileCache, TileCacheStats, TileContentHandle, TileContentSlot,
    TileContentState, TileGrid, TileId, TilePlan, TileResourceDescriptor, TileResourceKind,
    TileResourcePool, TileResourcePoolDelta,
};

#[cfg(test)]
mod tests;
