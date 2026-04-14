use zeno_core::{Backend, Platform, Size};

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSurface {
    pub id: String,
    pub platform: Platform,
    pub size: Size,
    pub scale_factor: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameReport {
    pub backend: Backend,
    pub command_count: usize,
    pub resource_count: usize,
    pub block_count: usize,
    pub display_item_count: usize,
    pub stacking_context_count: usize,
    pub damage_rect_count: usize,
    pub damage_full: bool,
    pub dirty_tile_count: usize,
    pub cached_tile_count: usize,
    pub reraster_tile_count: usize,
    pub raster_batch_tile_count: usize,
    pub composite_tile_count: usize,
    pub compositor_layer_count: usize,
    pub offscreen_layer_count: usize,
    pub tile_content_handle_count: usize,
    pub compositor_task_count: usize,
    pub compositor_queue_depth: usize,
    pub compositor_dropped_frame_count: usize,
    pub compositor_processed_frame_count: usize,
    pub released_tile_resource_count: usize,
    pub evicted_tile_resource_count: usize,
    pub budget_evicted_tile_resource_count: usize,
    pub age_evicted_tile_resource_count: usize,
    pub descriptor_limit_evicted_tile_resource_count: usize,
    pub reused_tile_resource_count: usize,
    pub reusable_tile_resource_count: usize,
    pub reusable_tile_resource_bytes: usize,
    pub tile_resource_reuse_budget_bytes: usize,
    pub compositor_worker_threaded: bool,
    pub compositor_worker_alive: bool,
    pub composite_executed_layer_count: usize,
    pub composite_executed_tile_count: usize,
    pub composite_offscreen_step_count: usize,
    pub surface_id: String,
}
