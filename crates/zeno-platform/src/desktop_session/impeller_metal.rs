use std::{collections::{BTreeSet, HashMap}, rc::Rc, time::Instant};

#[allow(deprecated)]
use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use metal::{Device, MTLPixelFormat, MetalLayer};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;
use zeno_backend_impeller::{CompositeParams, CompositeTextureTile, MetalSceneRenderer};
use zeno_core::{Backend, Color, Rect, Size, ZenoError, ZenoErrorCode, zeno_session_log};
use zeno_scene::{
    CompositeExecutor, CompositorPlanner, CompositorService, DisplayList, FrameReport,
    RenderSurface, SceneBlendMode, TileCache, TileContentHandle, TileGrid, TileResourcePool,
};

use super::{default_clear_color, desktop_session_error};
use crate::NativeSurface;

pub(super) struct ImpellerMetalSession {
    window: Rc<Window>,
    layer: MetalLayer,
    renderer: MetalSceneRenderer,
    surface: RenderSurface,
    tile_cache: TileCache,
    resource_pool: TileResourcePool,
    compositor_service: CompositorService,
    composite_executor: CompositeExecutor,
    tile_textures: HashMap<TileContentHandle, metal::Texture>,
    clear_color: Color,
}

impl ImpellerMetalSession {
    pub(super) fn new(
        event_loop: &ActiveEventLoop,
        config: &zeno_core::WindowConfig,
        native_surface: &NativeSurface,
    ) -> Result<Self, String> {
        let window_attributes = Window::default_attributes()
            .with_title(config.title.clone())
            .with_inner_size(LogicalSize::new(
                f64::from(config.size.width),
                f64::from(config.size.height),
            ))
            .with_transparent(config.transparent);
        let window = Rc::new(
            event_loop
                .create_window(window_attributes)
                .map_err(|error| error.to_string())?,
        );
        let device = Device::system_default()
            .ok_or_else(|| "metal device is unavailable on this mac".to_string())?;
        let queue = device.new_command_queue();
        let layer = MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        layer.set_presents_with_transaction(false);
        layer.set_framebuffer_only(true);
        layer.set_display_sync_enabled(true);
        layer.set_maximum_drawable_count(3);
        layer.set_opaque(!config.transparent);
        layer.set_contents_scale(window.scale_factor());
        attach_metal_layer(&window, &layer)?;
        let size = window.inner_size();
        layer.set_drawable_size(CGSize::new(size.width as f64, size.height as f64));
        let renderer = MetalSceneRenderer::new(device.clone(), queue.clone())
            .map_err(|error| error.to_string())?;
        let surface = native_surface.surface.clone();

        Ok(Self {
            window,
            layer,
            renderer,
            surface,
            tile_cache: TileCache::new(),
            resource_pool: TileResourcePool::new(),
            compositor_service: CompositorService::new(),
            composite_executor: CompositeExecutor::new(),
            tile_textures: HashMap::new(),
            clear_color: default_clear_color(config.transparent),
        })
    }

    pub(super) fn window(&self) -> &Window {
        self.window.as_ref()
    }

    pub(super) fn surface(&self) -> &RenderSurface {
        &self.surface
    }

    pub(super) fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError> {
        self.layer
            .set_drawable_size(CGSize::new(width.max(1) as f64, height.max(1) as f64));
        self.surface.size = Size::new(width.max(1) as f32, height.max(1) as f32);
        Ok(())
    }

    pub(super) fn submit_compositor_frame(
        &mut self,
        frame: &zeno_scene::CompositorFrame<DisplayList>,
    ) -> Result<FrameReport, ZenoError> {
        let submit_started = Instant::now();
        let display_list = &frame.payload;
        let worker_started = Instant::now();
        let worker_output = self
            .compositor_service
            .submit_frame(
                frame.generation,
                CompositorPlanner::new().plan(display_list, &mut self.tile_cache, &frame.damage),
            )
            .map_err(|error| {
                desktop_session_error(
                    ZenoErrorCode::SessionCreateRenderSessionFailed,
                    "compositor_worker",
                    error,
                )
            })?;
        let worker_ms = worker_started.elapsed().as_secs_f64() * 1000.0;
        let scheduled = &worker_output.scheduled;
        let submission = &scheduled.submission;
        let scheduler_stats = worker_output.scheduler_stats;
        let service_stats = self.compositor_service.stats();
        let tile_grid = TileGrid::for_viewport(display_list.viewport);
        let composite_plan_started = Instant::now();
        let composite_plan = self
            .composite_executor
            .plan(&submission.composite_pass, tile_grid);
        let composite_plan_ms = composite_plan_started.elapsed().as_secs_f64() * 1000.0;
        let composite_stats = composite_plan.stats;
        let resource_sync_started = Instant::now();
        let pool_delta = self.resource_pool.synchronize(&mut self.tile_cache);
        let resource_sync_ms = resource_sync_started.elapsed().as_secs_f64() * 1000.0;
        let eviction_stats = self.tile_cache.eviction_stats();
        for (handle, descriptor) in &pool_delta.allocated {
            if !self.tile_textures.contains_key(handle) {
                let texture = self
                    .renderer
                    .create_tile_texture(descriptor.width, descriptor.height);
                self.tile_textures.insert(*handle, texture);
            }
        }
        let released_tile_resource_count = pool_delta.released.len();
        let evicted_tile_resource_count = pool_delta.evicted.len();
        let reused_tile_resource_count = pool_delta.reused.len();
        for handle in pool_delta.released {
            self.tile_textures.remove(&handle);
        }
        let raster_bounds = submission.raster_batch.bounds();
        let drawable_started = Instant::now();
        let drawable = self.layer.next_drawable().ok_or_else(|| {
            desktop_session_error(
                ZenoErrorCode::SessionNextDrawableUnavailable,
                "next_drawable",
                "metal layer did not provide a drawable",
            )
        })?;
        let drawable_ms = drawable_started.elapsed().as_secs_f64() * 1000.0;
        zeno_session_log!(
            trace,
            op = "submit_compositor_frame",
            backend = ?Backend::Impeller,
            raster_mode = if submission.raster_batch.full_raster { "full" } else { "partial" },
            composite_mode = if submission.composite_pass.full_present { "full" } else { "partial" },
            raster_executor = "merged-region",
            surface = %self.surface.id,
            scale_factor = self.window.scale_factor(),
            clear = ?self.clear_color,
            ?raster_bounds,
            dirty_tiles = submission.tile_plan.stats.reraster_tile_count,
            cached_tiles = submission.tile_plan.stats.cached_tile_count,
            raster_tiles = submission.raster_batch.tile_count(),
            composite_layers = submission.composite_pass.layer_count(),
            composite_tiles = submission.composite_pass.tile_count(),
            compositor_layers = submission.layer_tree.layer_count(),
            offscreen_layers = submission.layer_tree.offscreen_layer_count(),
            tile_handles = self.tile_cache.content_handle_count(),
            tile_textures = self.tile_textures.len(),
            released_tile_resources = released_tile_resource_count,
            evicted_tile_resources = evicted_tile_resource_count,
            budget_evicted_tile_resources = eviction_stats.budget_eviction_count,
            age_evicted_tile_resources = eviction_stats.age_eviction_count,
            descriptor_limit_evicted_tile_resources = eviction_stats.descriptor_limit_eviction_count,
            reused_tile_resources = reused_tile_resource_count,
            reusable_tile_resources = self.tile_cache.reusable_handle_count(),
            reusable_tile_resource_bytes = self.tile_cache.reusable_byte_count(),
            tile_resource_reuse_budget_bytes = self.tile_cache.reuse_budget_byte_count(),
            compositor_tasks = scheduled.tasks.len(),
            compositor_queue_depth = scheduled.enqueued_frame_count.max(scheduler_stats.pending_frame_count),
            compositor_stale_frames = scheduled.stale_frame_count,
            compositor_dropped_frames = scheduled.dropped_frame_count,
            compositor_submitted_frames = service_stats.submitted_frame_count,
            compositor_processed_frames = service_stats.processed_frame_count,
            compositor_worker_threaded = service_stats.worker_threaded,
            compositor_worker_alive = service_stats.worker_alive,
            composite_executed_layers = composite_stats.executed_layer_count,
            composite_executed_tiles = composite_stats.executed_tile_count,
            composite_offscreen_steps = composite_stats.offscreen_step_count,
            items = display_list.items.len(),
            contexts = display_list.stacking_contexts.len(),
            generation = frame.generation,
            "impeller macos compositor frame submit"
        );
        self.renderer.begin_frame();
        let raster_started = Instant::now();
        let mut raster_tile_count = 0usize;
        for raster_tile in &submission.raster_batch.tiles {
            let Some(texture) = self.tile_textures.get(&raster_tile.content_handle) else {
                continue;
            };
            self.renderer.render_display_list_to_texture_tile(
                texture,
                display_list,
                &submission.layer_tree,
                raster_tile.rect,
            )?;
            raster_tile_count += 1;
        }
        let raster_ms = raster_started.elapsed().as_secs_f64() * 1000.0;
        let build_tiles_started = Instant::now();
        let composite_tiles = build_drawable_composite_tiles(
            composite_plan.jobs.iter().map(|job| (job.content_handle, job.rect)),
            &self.tile_textures,
        );
        let build_tiles_ms = build_tiles_started.elapsed().as_secs_f64() * 1000.0;
        let drawable_submit_started = Instant::now();
        if submission.raster_batch.full_raster && composite_tiles.is_empty() {
            self.renderer.render_display_list_to_drawable(
                drawable,
                display_list,
                Some(self.clear_color),
            )?;
        } else {
            self.renderer.composite_tile_textures_to_drawable(
                drawable,
                Some(self.clear_color),
                &composite_tiles,
                display_list.viewport.width.max(1.0),
                display_list.viewport.height.max(1.0),
            )?;
        }
        let drawable_submit_ms = drawable_submit_started.elapsed().as_secs_f64() * 1000.0;
        let total_submit_ms = submit_started.elapsed().as_secs_f64() * 1000.0;
        let (offscreen_cache_entries, offscreen_cache_hits, offscreen_cache_misses) =
            self.renderer.offscreen_context_cache_stats();
        // Stable perf instrumentation. Keep op names in sync with
        // docs/architecture/performance-debugging.md.
        // #region debug-point impeller-submit-timing
        zeno_session_log!(
            trace,
            op = "impeller_submit_timing",
            total_submit_ms,
            worker_ms,
            composite_plan_ms,
            resource_sync_ms,
            drawable_ms,
            raster_ms,
            raster_tile_count,
            build_tiles_ms,
            drawable_submit_ms,
            reraster_tile_count = submission.tile_plan.stats.reraster_tile_count,
            composite_tile_count = submission.composite_pass.tile_count(),
            offscreen_layer_count = submission.layer_tree.offscreen_layer_count(),
            offscreen_cache_entries,
            offscreen_cache_hits,
            offscreen_cache_misses,
            "impeller submit timing"
        );
        // #endregion
        Ok(FrameReport {
            backend: Backend::Impeller,
            command_count: display_list.items.len(),
            resource_count: self.resource_pool.resource_count(),
            block_count: 0,
            display_item_count: display_list.items.len(),
            stacking_context_count: display_list.stacking_contexts.len(),
            damage_rect_count: frame.damage.rect_count(),
            damage_full: frame.damage.is_full(),
            dirty_tile_count: submission.tile_plan.stats.reraster_tile_count,
            cached_tile_count: submission.tile_plan.stats.cached_tile_count,
            reraster_tile_count: submission.tile_plan.stats.reraster_tile_count,
            raster_batch_tile_count: submission.raster_batch.tile_count(),
            composite_tile_count: submission.composite_pass.tile_count(),
            compositor_layer_count: submission.layer_tree.layer_count(),
            offscreen_layer_count: submission.layer_tree.offscreen_layer_count(),
            tile_content_handle_count: self.tile_cache.content_handle_count(),
            compositor_task_count: scheduled.tasks.len(),
            compositor_queue_depth: scheduled.enqueued_frame_count,
            compositor_dropped_frame_count: service_stats.dropped_frame_count,
            compositor_processed_frame_count: service_stats.processed_frame_count,
            released_tile_resource_count,
            evicted_tile_resource_count,
            budget_evicted_tile_resource_count: eviction_stats.budget_eviction_count,
            age_evicted_tile_resource_count: eviction_stats.age_eviction_count,
            descriptor_limit_evicted_tile_resource_count: eviction_stats
                .descriptor_limit_eviction_count,
            reused_tile_resource_count,
            reusable_tile_resource_count: self.tile_cache.reusable_handle_count(),
            reusable_tile_resource_bytes: self.tile_cache.reusable_byte_count(),
            tile_resource_reuse_budget_bytes: self.tile_cache.reuse_budget_byte_count(),
            compositor_worker_threaded: service_stats.worker_threaded,
            compositor_worker_alive: service_stats.worker_alive,
            composite_executed_layer_count: composite_stats.executed_layer_count,
            composite_executed_tile_count: composite_stats.executed_tile_count,
            composite_offscreen_step_count: composite_stats.offscreen_step_count,
            surface_id: self.surface.id.clone(),
        })
    }

    pub(super) fn cache_summary(&self) -> String {
        format!(
            "clear:{:?} scale:{:.2}",
            self.clear_color,
            self.window.scale_factor()
        )
    }
}

fn build_drawable_composite_tiles<'a>(
    jobs: impl IntoIterator<Item = (TileContentHandle, Rect)>,
    tile_textures: &'a HashMap<TileContentHandle, metal::Texture>,
) -> Vec<CompositeTextureTile<'a>> {
    unique_tile_jobs(jobs)
        .into_iter()
        .filter_map(|(content_handle, rect)| {
            let texture = tile_textures.get(&content_handle)?;
            // Tile textures already contain the fully composited scene for that tile. The final
            // drawable pass must only blit each tile once instead of reapplying layer effects.
            Some(CompositeTextureTile {
                texture,
                rect,
                opacity: 1.0,
                blend_mode: SceneBlendMode::Normal,
                params: CompositeParams::default(),
            })
        })
        .collect()
}

fn unique_tile_jobs(
    jobs: impl IntoIterator<Item = (TileContentHandle, Rect)>,
) -> Vec<(TileContentHandle, Rect)> {
    let mut seen_handles = BTreeSet::new();
    let mut unique = Vec::new();
    for (content_handle, rect) in jobs {
        if seen_handles.insert(content_handle) {
            unique.push((content_handle, rect));
        }
    }
    unique
}

#[allow(deprecated)]
fn attach_metal_layer(window: &Window, layer: &MetalLayer) -> Result<(), String> {
    let raw = window
        .window_handle()
        .map_err(|error| error.to_string())?
        .as_raw();
    unsafe {
        match raw {
            RawWindowHandle::AppKit(handle) => {
                let view = handle.ns_view.as_ptr() as cocoa_id;
                view.setWantsLayer(true);
                view.setLayer(std::mem::transmute(layer.as_ref()));
                Ok(())
            }
            _ => Err("window is not backed by an AppKit NSView".to_string()),
        }
    }
}
