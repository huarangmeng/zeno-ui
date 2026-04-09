use std::{ffi::CString, num::NonZeroU32, rc::Rc};

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
use zeno_backend_skia::{render_scene_region_to_canvas, render_scene_to_canvas, SkiaTextCache};
use zeno_core::{zeno_session_log, Backend, Color, Platform, Size, ZenoError, ZenoErrorCode};
use zeno_graphics::{FrameReport, RenderSurface, Scene, SceneSubmit};

use super::desktop_session_error;
use super::scene::{default_clear_color, ensure_clear_command, patch_stats};

pub(super) struct SkiaGlSession {
    window: Rc<Window>,
    surface: RenderSurface,
    gl_config: Config,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    gr_context: sk::gpu::DirectContext,
    text_cache: SkiaTextCache,
    clear_color: Color,
    last_scene: Option<Scene>,
}

impl SkiaGlSession {
    pub(super) fn new(event_loop: &ActiveEventLoop, config: &zeno_core::WindowConfig) -> Result<Self, String> {
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

        let surface = RenderSurface {
            id: "skia-gl-surface".to_string(),
            platform: Platform::current(),
            size: Size::new(config.size.width, config.size.height),
            scale_factor: config.scale_factor,
        };

        Ok(Self {
            window,
            surface,
            gl_config,
            gl_context,
            gl_surface,
            gr_context,
            text_cache: SkiaTextCache::default(),
            clear_color: default_clear_color(config.transparent),
            last_scene: None,
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

    pub(super) fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError> {
        let scene = submit.snapshot(self.last_scene.as_ref()).ok_or_else(|| {
            desktop_session_error(
                ZenoErrorCode::GraphicsScenePatchWithoutBase,
                "submit_scene",
                "scene patch requires a previous snapshot",
            )
        })?;
        let scene = ensure_clear_command(&scene, self.clear_color);
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
                "render_scene",
                "failed to wrap GL render target",
            )
        })?;

        let dirty_bounds = match submit {
            SceneSubmit::Full(_) => None,
            SceneSubmit::Patch { patch, .. } if self.last_scene.is_some() => {
                patch.dirty_bounds(self.last_scene.as_ref())
            }
            SceneSubmit::Patch { .. } => None,
        };
        zeno_session_log!(
            trace,
            op = "submit_scene",
            backend = ?Backend::Skia,
            mode = if dirty_bounds.is_some() { "patch" } else { "full" },
            surface = %self.surface.id,
            scale_factor = self.window.scale_factor(),
            clear = ?self.clear_color,
            ?dirty_bounds,
            "skia macos scene submit"
        );
        if let Some(bounds) = dirty_bounds {
            render_scene_region_to_canvas(surface.canvas(), &scene, bounds, &mut self.text_cache);
        } else {
            render_scene_to_canvas(surface.canvas(), &scene, &mut self.text_cache);
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
        let (patch_upserts, patch_removes) = patch_stats(submit);
        Ok(FrameReport {
            backend: Backend::Skia,
            command_count: scene.commands.len(),
            resource_count: scene.resource_keys().len(),
            block_count: scene.blocks.len(),
            patch_upserts,
            patch_removes,
            surface_id: self.surface.id.clone(),
        })
        .map(|report| {
            self.last_scene = Some(scene);
            report
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

fn create_not_current_context(window: &Window, gl_config: &Config) -> Result<NotCurrentContext, String> {
    let raw_window_handle = window.window_handle().map_err(|error| error.to_string())?.as_raw();
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

fn create_gl_surface(window: &Window, gl_config: &Config) -> Result<Surface<WindowSurface>, String> {
    let raw_window_handle = window.window_handle().map_err(|error| error.to_string())?.as_raw();
    let gl_display = gl_config.display();
    let size = window.inner_size();
    let width = NonZeroU32::new(size.width.max(1)).ok_or_else(|| "invalid window width".to_string())?;
    let height = NonZeroU32::new(size.height.max(1)).ok_or_else(|| "invalid window height".to_string())?;
    let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(raw_window_handle, width, height);
    let surface =
        unsafe { gl_display.create_window_surface(gl_config, &attrs) }.map_err(|error| error.to_string())?;
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
