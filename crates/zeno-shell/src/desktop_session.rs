use std::{ffi::CString, num::NonZeroU32, rc::Rc};

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
#[allow(deprecated)]
use cocoa::{appkit::NSView, base::id as cocoa_id};
#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
use core_graphics_types::geometry::CGSize;
#[cfg(feature = "desktop_winit")]
use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
#[cfg(feature = "desktop_winit")]
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext, Version,
};
#[cfg(feature = "desktop_winit")]
use glutin::display::{GetGlDisplay, GlDisplay};
#[cfg(feature = "desktop_winit")]
use glutin::prelude::*;
#[cfg(feature = "desktop_winit")]
use glutin::surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};
#[cfg(feature = "desktop_winit")]
use glutin_winit::DisplayBuilder;
#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
use metal::{Device, MTLPixelFormat, MetalLayer};
#[cfg(feature = "desktop_winit")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(feature = "desktop_winit")]
use skia_safe as sk;
#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
use zeno_backend_impeller::MetalSceneRenderer;
#[cfg(feature = "desktop_winit")]
use zeno_backend_skia::{render_scene_to_canvas, SkiaTextCache};
use zeno_core::{Backend, Platform, Size, WindowConfig, ZenoError, ZenoErrorCode};
use zeno_graphics::{FrameReport, RenderCapabilities, RenderSession, RenderSurface, Scene, SceneSubmit};
use zeno_runtime::ResolvedSession;
#[cfg(feature = "desktop_winit")]
use winit::dpi::LogicalSize;
#[cfg(feature = "desktop_winit")]
use winit::event_loop::ActiveEventLoop;
#[cfg(feature = "desktop_winit")]
use winit::window::Window;

#[cfg(feature = "desktop_winit")]
pub trait DesktopRenderSessionHandle: RenderSession {
    fn window(&self) -> &Window;

    fn cache_summary(&self) -> String;
}

#[cfg(feature = "desktop_winit")]
pub type BoxedDesktopRenderSession = Box<dyn DesktopRenderSessionHandle>;

fn desktop_session_error(
    code: ZenoErrorCode,
    operation: &'static str,
    message: impl Into<String>,
) -> ZenoError {
    ZenoError::invalid_configuration(code, "shell.desktop_session", operation, message)
}

#[cfg(feature = "desktop_winit")]
pub fn create_desktop_render_session(
    resolved: &ResolvedSession,
    event_loop: &ActiveEventLoop,
) -> Result<BoxedDesktopRenderSession, ZenoError> {
    DesktopRenderSession::new(event_loop, resolved)
        .map(|session| Box::new(session) as BoxedDesktopRenderSession)
        .map_err(|error| {
            desktop_session_error(ZenoErrorCode::SessionCreateRenderSessionFailed, "create_render_session", error)
        })
}

#[cfg(feature = "desktop_winit")]
enum DesktopRenderSession {
    Skia(SkiaGlSession),
    #[cfg(target_os = "macos")]
    Impeller(ImpellerMetalSession),
}

#[cfg(feature = "desktop_winit")]
impl DesktopRenderSession {
    fn new(event_loop: &ActiveEventLoop, resolved: &ResolvedSession) -> Result<Self, String> {
        match resolved.backend.backend_kind {
            Backend::Skia => SkiaGlSession::new(event_loop, &resolved.window).map(Self::Skia),
            #[cfg(target_os = "macos")]
            Backend::Impeller => ImpellerMetalSession::new(event_loop, &resolved.window).map(Self::Impeller),
            #[cfg(not(target_os = "macos"))]
            Backend::Impeller => {
                Err("impeller desktop presenter is not implemented for this platform".to_string())
            }
        }
    }
}

#[cfg(feature = "desktop_winit")]
impl DesktopRenderSessionHandle for DesktopRenderSession {
    fn window(&self) -> &Window {
        match self {
            Self::Skia(session) => session.window.as_ref(),
            #[cfg(target_os = "macos")]
            Self::Impeller(session) => session.window.as_ref(),
        }
    }

    fn cache_summary(&self) -> String {
        match self {
            Self::Skia(session) => session.cache_summary(),
            #[cfg(target_os = "macos")]
            Self::Impeller(_) => "none".to_string(),
        }
    }
}

#[cfg(feature = "desktop_winit")]
impl RenderSession for DesktopRenderSession {
    fn kind(&self) -> Backend {
        match self {
            Self::Skia(_) => Backend::Skia,
            #[cfg(target_os = "macos")]
            Self::Impeller(_) => Backend::Impeller,
        }
    }

    fn capabilities(&self) -> RenderCapabilities {
        RenderCapabilities {
            gpu_compositing: true,
            text_shaping: true,
            filters: true,
            offscreen_rendering: false,
        }
    }

    fn surface(&self) -> &RenderSurface {
        match self {
            Self::Skia(session) => &session.surface,
            #[cfg(target_os = "macos")]
            Self::Impeller(session) => &session.surface,
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError> {
        match self {
            Self::Skia(session) => session.resize(width, height),
            #[cfg(target_os = "macos")]
            Self::Impeller(session) => session.resize(width, height),
        }
    }

    fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError> {
        match self {
            Self::Skia(session) => session.submit_scene(submit),
            #[cfg(target_os = "macos")]
            Self::Impeller(session) => session.submit_scene(submit),
        }
    }
}

#[cfg(feature = "desktop_winit")]
struct SkiaGlSession {
    window: Rc<Window>,
    surface: RenderSurface,
    gl_config: Config,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    gr_context: sk::gpu::DirectContext,
    text_cache: SkiaTextCache,
    last_scene: Option<Scene>,
}

#[cfg(feature = "desktop_winit")]
impl SkiaGlSession {
    fn new(event_loop: &ActiveEventLoop, config: &WindowConfig) -> Result<Self, String> {
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
                SwapInterval::Wait(NonZeroU32::new(1).ok_or_else(|| "invalid vsync interval".to_string())?),
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
            last_scene: None,
        })
    }

    fn resize(&self, width: u32, height: u32) -> Result<(), ZenoError> {
        let width = NonZeroU32::new(width.max(1))
            .ok_or_else(|| {
                desktop_session_error(ZenoErrorCode::SessionInvalidWindowWidth, "resize", "invalid window width")
            })?;
        let height = NonZeroU32::new(height.max(1))
            .ok_or_else(|| {
                desktop_session_error(ZenoErrorCode::SessionInvalidWindowHeight, "resize", "invalid window height")
            })?;
        self.gl_surface.resize(&self.gl_context, width, height);
        Ok(())
    }

    fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError> {
        let scene = submit.snapshot(self.last_scene.as_ref()).ok_or_else(|| {
            desktop_session_error(
                ZenoErrorCode::GraphicsScenePatchWithoutBase,
                "submit_scene",
                "scene patch requires a previous snapshot",
            )
        })?;
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

        render_scene_to_canvas(surface.canvas(), &scene, &mut self.text_cache);
        self.gr_context.flush_and_submit();
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .map_err(|error| {
                desktop_session_error(ZenoErrorCode::SessionSwapBuffersFailed, "swap_buffers", error.to_string())
            })?;
        Ok(FrameReport {
            backend: Backend::Skia,
            command_count: scene.commands.len(),
            resource_count: scene.resource_keys().len(),
            surface_id: self.surface.id.clone(),
        })
        .map(|report| {
            self.last_scene = Some(scene);
            report
        })
    }

    fn cache_summary(&self) -> String {
        let stats = self.text_cache.stats();
        format!(
            "fonts:{} typefaces:{} font_hits:{} typeface_hits:{}",
            stats.cached_fonts,
            stats.cached_typefaces,
            stats.font_hits,
            stats.typeface_hits
        )
    }
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
struct ImpellerMetalSession {
    window: Rc<Window>,
    layer: MetalLayer,
    renderer: MetalSceneRenderer,
    surface: RenderSurface,
    last_scene: Option<Scene>,
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
impl ImpellerMetalSession {
    fn new(event_loop: &ActiveEventLoop, config: &WindowConfig) -> Result<Self, String> {
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
        let device =
            Device::system_default().ok_or_else(|| "metal device is unavailable on this mac".to_string())?;
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
        let renderer = MetalSceneRenderer::new(device.clone(), queue.clone()).map_err(|error| error.to_string())?;
        let surface = RenderSurface {
            id: "impeller-metal-surface".to_string(),
            platform: Platform::current(),
            size: Size::new(config.size.width, config.size.height),
            scale_factor: config.scale_factor,
        };

        Ok(Self {
            window,
            layer,
            renderer,
            surface,
            last_scene: None,
        })
    }

    fn resize(&self, width: u32, height: u32) -> Result<(), ZenoError> {
        self.layer
            .set_drawable_size(CGSize::new(width.max(1) as f64, height.max(1) as f64));
        Ok(())
    }

    fn submit_scene(&mut self, submit: &SceneSubmit) -> Result<FrameReport, ZenoError> {
        let scene = submit.snapshot(self.last_scene.as_ref()).ok_or_else(|| {
            desktop_session_error(
                ZenoErrorCode::GraphicsScenePatchWithoutBase,
                "submit_scene",
                "scene patch requires a previous snapshot",
            )
        })?;
        let drawable = self.layer.next_drawable().ok_or_else(|| {
            desktop_session_error(
                ZenoErrorCode::SessionNextDrawableUnavailable,
                "next_drawable",
                "metal layer did not provide a drawable",
            )
        })?;
        self.renderer.render_to_drawable(drawable, &scene)?;
        Ok(FrameReport {
            backend: Backend::Impeller,
            command_count: scene.commands.len(),
            resource_count: scene.resource_keys().len(),
            surface_id: self.surface.id.clone(),
        })
        .map(|report| {
            self.last_scene = Some(scene);
            report
        })
    }
}

#[cfg(all(target_os = "macos", feature = "desktop_winit"))]
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

#[cfg(feature = "desktop_winit")]
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

#[cfg(feature = "desktop_winit")]
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

#[cfg(feature = "desktop_winit")]
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
