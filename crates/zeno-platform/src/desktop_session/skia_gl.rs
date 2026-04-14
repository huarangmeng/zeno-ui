use std::{collections::HashMap, ffi::CString, num::NonZeroU32, rc::Rc};

use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext, Version,
};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::prelude::*;
use glutin::surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use skia_safe as sk;
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;
use zeno_backend_skia::{
    SkiaTextCache, render_display_list_tile_to_canvas,
};
use zeno_core::{Backend, Color, Size, ZenoError, ZenoErrorCode, zeno_session_log};
use zeno_scene::{
    CompositeExecutor, CompositeLayerJob, CompositorBlendMode, CompositorEffect,
    CompositorService, DisplayList, FrameReport, RenderSurface, TileCache, TileContentHandle,
    TileGrid, TileResourcePool,
};

use super::{default_clear_color, desktop_session_error};
use crate::NativeSurface;

pub(super) struct SkiaGlSession {
    window: Rc<Window>,
    surface: RenderSurface,
    gl_config: Config,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    gr_context: sk::gpu::DirectContext,
    text_cache: SkiaTextCache,
    tile_cache: TileCache,
    resource_pool: TileResourcePool,
    compositor_service: CompositorService,
    composite_executor: CompositeExecutor,
    tile_surfaces: HashMap<TileContentHandle, sk::Surface>,
    clear_color: Color,
}

impl SkiaGlSession {
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

        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_transparency(config.transparent);
        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));

        let (window, gl_config) = display_builder
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|accum, config| {
                        if config.num_samples() > accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .expect("at least one GL config")
            })
            .map_err(|error| error.to_string())?;

        let window = Rc::new(window.ok_or_else(|| "glutin did not create a window".to_string())?);
        let not_current_context = create_not_current_context(&window, &gl_config)?;
        let gl_surface = create_gl_surface(&window, &gl_config)?;
        let gl_context = not_current_context
            .make_current(&gl_surface)
            .map_err(|error| error.to_string())?;
        gl_surface
            .set_swap_interval(
                &gl_context,
                SwapInterval::Wait(
                    NonZeroU32::new(1).ok_or_else(|| "invalid vsync interval".to_string())?,
                ),
            )
            .ok();
        let gr_context = create_gr_context(&gl_config)?;

        let surface = native_surface.surface.clone();

        Ok(Self {
            window,
            surface,
            gl_config,
            gl_context,
            gl_surface,
            gr_context,
            text_cache: SkiaTextCache::default(),
            tile_cache: TileCache::new(),
            resource_pool: TileResourcePool::new(),
            compositor_service: CompositorService::new(),
            composite_executor: CompositeExecutor::new(),
            tile_surfaces: HashMap::new(),
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
        let width = NonZeroU32::new(width.max(1)).ok_or_else(|| {
            desktop_session_error(
                ZenoErrorCode::SessionInvalidWindowWidth,
                "resize",
                "invalid window width",
            )
        })?;
        let height = NonZeroU32::new(height.max(1)).ok_or_else(|| {
            desktop_session_error(
                ZenoErrorCode::SessionInvalidWindowHeight,
                "resize",
                "invalid window height",
            )
        })?;
        self.gl_surface.resize(&self.gl_context, width, height);
        self.surface.size = Size::new(width.get() as f32, height.get() as f32);
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
            let size = (
                i32::try_from(descriptor.width.max(1)).unwrap_or(1),
                i32::try_from(descriptor.height.max(1)).unwrap_or(1),
            );
            self.tile_surfaces.entry(*handle).or_insert_with(|| {
                sk::surfaces::raster_n32_premul(size).expect("tile offscreen surface should allocate")
            });
        }
        let released_tile_resource_count = pool_delta.released.len();
        let evicted_tile_resource_count = pool_delta.evicted.len();
        let reused_tile_resource_count = pool_delta.reused.len();
        for handle in pool_delta.released {
            self.tile_surfaces.remove(&handle);
        }
        let raster_bounds = submission.raster_batch.bounds();
        let size = self.window.inner_size();
        let (width, height) = (size.width.max(1), size.height.max(1));
        self.resize(width, height)?;

        let mut framebuffer_binding = 0;
        unsafe {
            gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut framebuffer_binding);
        }

        let framebuffer_info = sk::gpu::gl::FramebufferInfo {
            fboid: framebuffer_binding as u32,
            format: gl::RGBA8,
            protected: sk::gpu::Protected::No,
        };
        let backend_render_target = sk::gpu::backend_render_targets::make_gl(
            (width as i32, height as i32),
            self.gl_config.num_samples() as usize,
            self.gl_config.stencil_size() as usize,
            framebuffer_info,
        );
        let mut surface = sk::gpu::surfaces::wrap_backend_render_target(
            &mut self.gr_context,
            &backend_render_target,
            sk::gpu::SurfaceOrigin::BottomLeft,
            sk::ColorType::RGBA8888,
            None,
            None,
        )
        .ok_or_else(|| {
            desktop_session_error(
                ZenoErrorCode::SessionWrapRenderTargetFailed,
                "render_display_list",
                "failed to wrap GL render target",
            )
        })?;
        zeno_session_log!(
            trace,
            op = "submit_compositor_frame",
            backend = ?Backend::Skia,
            raster_mode = if submission.raster_batch.full_raster { "full" } else { "partial" },
            composite_mode = if submission.composite_pass.full_present { "full" } else { "partial" },
            raster_executor = if submission.raster_batch.full_raster { "full-pass" } else { "per-tile" },
            surface = %self.surface.id,
            scale_factor = self.window.scale_factor(),
            ?raster_bounds,
            dirty_tiles = submission.tile_plan.stats.reraster_tile_count,
            cached_tiles = submission.tile_plan.stats.cached_tile_count,
            raster_tiles = submission.raster_batch.tile_count(),
            composite_layers = submission.composite_pass.layer_count(),
            composite_tiles = submission.composite_pass.tile_count(),
            compositor_layers = submission.layer_tree.layer_count(),
            offscreen_layers = submission.layer_tree.offscreen_layer_count(),
            tile_handles = self.tile_cache.content_handle_count(),
            tile_surfaces = self.tile_surfaces.len(),
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
            generation = frame.generation,
            items = display_list.items.len(),
            stacking_contexts = display_list.stacking_contexts.len(),
            "skia macos compositor frame submit"
        );
        for raster_tile in &submission.raster_batch.tiles {
            let slot = self.tile_cache.content_slot(raster_tile.tile_id).ok_or_else(|| {
                desktop_session_error(
                    ZenoErrorCode::SessionWrapRenderTargetFailed,
                    "tile_cache_slot",
                    "missing tile content slot for raster tile",
                )
            })?;
            let size = (
                i32::try_from(slot.resource.width.max(1)).unwrap_or(1),
                i32::try_from(slot.resource.height.max(1)).unwrap_or(1),
            );
            if let std::collections::hash_map::Entry::Vacant(entry) =
                self.tile_surfaces.entry(raster_tile.content_handle)
            {
                let surface = sk::surfaces::raster_n32_premul(size)
                    .expect("tile offscreen surface should allocate");
                entry.insert(surface);
            }
            let surface = self
                .tile_surfaces
                .get_mut(&raster_tile.content_handle)
                .expect("tile surface allocated");
            render_display_list_tile_to_canvas(
                surface.canvas(),
                display_list,
                raster_tile.rect,
                &mut self.text_cache,
            );
        }
        if submission.raster_batch.full_raster {
            surface.canvas().clear(sk::Color::TRANSPARENT);
        } else {
            surface.canvas().clear(sk::Color::TRANSPARENT);
        }
        for layer in &composite_plan.layer_jobs {
            let layer_paint = compositor_layer_paint(layer);
            let draw_direct = !layer.needs_offscreen
                && layer.opacity >= 0.999
                && matches!(layer.blend_mode, CompositorBlendMode::Normal)
                && layer.effects.is_empty();
            if draw_direct {
                surface.canvas().save();
            } else {
                let bounds = sk::Rect::from_xywh(
                    layer.effect_bounds.origin.x,
                    layer.effect_bounds.origin.y,
                    layer.effect_bounds.size.width,
                    layer.effect_bounds.size.height,
                );
                let layer_rec = sk::canvas::SaveLayerRec::default()
                    .bounds(&bounds)
                    .paint(&layer_paint);
                surface.canvas().save_layer(&layer_rec);
            }
            for tile in composite_plan.jobs.iter().filter(|job| job.layer_id == layer.layer_id) {
                let Some(image) = self
                    .tile_surfaces
                    .get_mut(&tile.content_handle)
                    .map(|tile_surface| tile_surface.image_snapshot())
                else {
                    continue;
                };
                let dst = sk::Rect::from_xywh(
                    tile.rect.origin.x,
                    tile.rect.origin.y,
                    tile.rect.size.width,
                    tile.rect.size.height,
                );
                if draw_direct {
                    surface.canvas().draw_image_rect(image, None, dst, &layer_paint);
                } else {
                    let draw_paint = sk::Paint::default();
                    surface.canvas().draw_image_rect(image, None, dst, &draw_paint);
                }
            }
            surface.canvas().restore();
        }
        self.gr_context.flush_and_submit();
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .map_err(|error| {
                desktop_session_error(
                    ZenoErrorCode::SessionSwapBuffersFailed,
                    "swap_buffers",
                    error.to_string(),
                )
            })?;
        Ok(FrameReport {
            backend: Backend::Skia,
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
        let stats = self.text_cache.stats();
        format!(
            "fonts:{} typefaces:{} font_hits:{} typeface_hits:{} clear:{:?} scale:{:.2}",
            stats.cached_fonts,
            stats.cached_typefaces,
            stats.font_hits,
            stats.typeface_hits,
            self.clear_color,
            self.window.scale_factor()
        )
    }
}

fn compositor_layer_paint(layer: &CompositeLayerJob) -> sk::Paint {
    let mut paint = sk::Paint::default();
    paint.set_anti_alias(true);
    paint.set_alpha_f(layer.opacity.clamp(0.0, 1.0));
    paint.set_blend_mode(sk_blend_mode(layer.blend_mode));
    if let Some(filter) = compositor_image_filter(&layer.effects) {
        paint.set_image_filter(filter);
    }
    paint
}

fn sk_blend_mode(mode: CompositorBlendMode) -> sk::BlendMode {
    match mode {
        CompositorBlendMode::Normal => sk::BlendMode::SrcOver,
        CompositorBlendMode::Multiply => sk::BlendMode::Multiply,
        CompositorBlendMode::Screen => sk::BlendMode::Screen,
    }
}

fn compositor_image_filter(effects: &[CompositorEffect]) -> Option<sk::ImageFilter> {
    let mut current = None;
    for effect in effects {
        current = match effect {
            CompositorEffect::Blur { sigma } => {
                sk::image_filters::blur((*sigma, *sigma), None, current, None)
            }
            CompositorEffect::DropShadow {
                dx,
                dy,
                blur,
                color,
            } => sk::image_filters::drop_shadow(
                (*dx, *dy),
                (*blur, *blur),
                sk::Color4f::new(
                    f32::from(color.red) / 255.0,
                    f32::from(color.green) / 255.0,
                    f32::from(color.blue) / 255.0,
                    f32::from(color.alpha) / 255.0,
                ),
                None,
                current,
                None,
            ),
        };
    }
    current
}

fn create_not_current_context(
    window: &Window,
    gl_config: &Config,
) -> Result<NotCurrentContext, String> {
    let raw_window_handle = window
        .window_handle()
        .map_err(|error| error.to_string())?
        .as_raw();
    let gl_display = gl_config.display();
    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .build(Some(raw_window_handle));
    let fallback_context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(None))
        .build(Some(raw_window_handle));

    unsafe {
        gl_display
            .create_context(gl_config, &context_attributes)
            .or_else(|_| gl_display.create_context(gl_config, &fallback_context_attributes))
    }
    .map_err(|error| error.to_string())
}

fn create_gl_surface(
    window: &Window,
    gl_config: &Config,
) -> Result<Surface<WindowSurface>, String> {
    let raw_window_handle = window
        .window_handle()
        .map_err(|error| error.to_string())?
        .as_raw();
    let gl_display = gl_config.display();
    let size = window.inner_size();
    let width =
        NonZeroU32::new(size.width.max(1)).ok_or_else(|| "invalid window width".to_string())?;
    let height =
        NonZeroU32::new(size.height.max(1)).ok_or_else(|| "invalid window height".to_string())?;
    let attrs =
        SurfaceAttributesBuilder::<WindowSurface>::new().build(raw_window_handle, width, height);
    let surface = unsafe { gl_display.create_window_surface(gl_config, &attrs) }
        .map_err(|error| error.to_string())?;
    Ok(surface)
}

fn create_gr_context(gl_config: &Config) -> Result<sk::gpu::DirectContext, String> {
    let gl_display = gl_config.display();
    gl::load_with(|name| {
        let name = CString::new(name).expect("GL symbol");
        gl_display.get_proc_address(name.as_c_str()) as *const _
    });
    let interface = sk::gpu::gl::Interface::new_load_with(|name| {
        let name = CString::new(name).expect("GL symbol");
        gl_display.get_proc_address(name.as_c_str()) as *const _
    })
    .ok_or_else(|| "failed to load Skia GL interface".to_string())?;
    sk::gpu::direct_contexts::make_gl(interface, None)
        .ok_or_else(|| "failed to create Skia GL direct context".to_string())
}
