use std::{collections::HashMap, rc::Rc};

#[allow(deprecated)]
use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use metal::{Device, MTLPixelFormat, MetalLayer};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;
use zeno_backend_impeller::{CompositeParams, CompositeTextureTile, MetalSceneRenderer};
use zeno_core::{Backend, Color, Size, ZenoError, ZenoErrorCode, zeno_session_log};
use zeno_scene::{
    CompositeExecutor, CompositeLayerJob, CompositorBlendMode, CompositorEffect,
    CompositorService, DisplayList, FrameReport, RenderSurface, SceneBlendMode, TileCache,
    TileContentHandle, TileGrid, TileResourcePool,
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
        let display_list = &frame.payload;
        let worker_output = self
            .compositor_service
            .submit_frame(
                frame.generation,
                display_list.build_compositor_submission(&mut self.tile_cache, &frame.damage),
            )
            .map_err(|error| {
                desktop_session_error(
                    ZenoErrorCode::SessionCreateRenderSessionFailed,
                    "compositor_worker",
                    error,
                )
            })?;
        let scheduled = &worker_output.scheduled;
        let submission = &scheduled.submission;
        let scheduler_stats = worker_output.scheduler_stats;
        let service_stats = self.compositor_service.stats();
        let tile_grid = TileGrid::for_viewport(display_list.viewport);
        let composite_plan = self.composite_executor.plan(&submission.composite_pass, tile_grid);
        let composite_stats = composite_plan.stats;
        let pool_delta = self.resource_pool.synchronize(&mut self.tile_cache);
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
        let drawable = self.layer.next_drawable().ok_or_else(|| {
            desktop_session_error(
                ZenoErrorCode::SessionNextDrawableUnavailable,
                "next_drawable",
                "metal layer did not provide a drawable",
            )
        })?;
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
        for raster_tile in &submission.raster_batch.tiles {
            let Some(texture) = self.tile_textures.get(&raster_tile.content_handle) else {
                continue;
            };
            self.renderer
                .render_display_list_to_texture_tile(texture, display_list, raster_tile.rect)?;
        }
        let composite_tiles = composite_plan
            .jobs
            .iter()
            .filter_map(|job| {
                let texture = self.tile_textures.get(&job.content_handle)?;
                Some(CompositeTextureTile {
                    texture,
                    rect: job.rect,
                    opacity: job.opacity,
                    blend_mode: layer_job_blend_mode(
                        composite_plan
                            .layer_jobs
                            .iter()
                            .find(|layer| layer.layer_id == job.layer_id)?,
                    ),
                    params: layer_job_composite_params(
                        composite_plan
                            .layer_jobs
                            .iter()
                            .find(|layer| layer.layer_id == job.layer_id)?,
                        job.rect,
                    ),
                })
            })
            .collect::<Vec<_>>();
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
            descriptor_limit_evicted_tile_resource_count: eviction_stats.descriptor_limit_eviction_count,
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

fn layer_job_blend_mode(layer: &CompositeLayerJob) -> SceneBlendMode {
    match layer.blend_mode {
        CompositorBlendMode::Normal => SceneBlendMode::Normal,
        CompositorBlendMode::Multiply => SceneBlendMode::Multiply,
        CompositorBlendMode::Screen => SceneBlendMode::Screen,
    }
}

fn layer_job_composite_params(layer: &CompositeLayerJob, rect: zeno_core::Rect) -> CompositeParams {
    let mut params = CompositeParams {
        inv_texture_size: [1.0 / rect.size.width.max(1.0), 1.0 / rect.size.height.max(1.0)],
        ..CompositeParams::default()
    };
    for effect in &layer.effects {
        match effect {
            CompositorEffect::Blur { sigma } => {
                params.blur_sigma = *sigma;
                params.flags |= 1;
            }
            CompositorEffect::DropShadow {
                dx,
                dy,
                blur,
                color,
            } => {
                params.shadow_blur = *blur;
                params.shadow_offset = [*dx, *dy];
                params.shadow_color = [
                    f32::from(color.red) / 255.0,
                    f32::from(color.green) / 255.0,
                    f32::from(color.blue) / 255.0,
                    f32::from(color.alpha) / 255.0,
                ];
                params.flags |= 2;
            }
        }
    }
    params
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
