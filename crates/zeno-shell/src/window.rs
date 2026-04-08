use std::{ffi::CString, num::NonZeroU32, rc::Rc};

use zeno_core::{Backend, WindowConfig, ZenoError};
use zeno_graphics::{DrawCommand, Scene};

use crate::shell::DesktopShell;

#[cfg(all(feature = "desktop_winit", target_os = "macos"))]
#[allow(deprecated)]
use cocoa::{appkit::NSView, base::id as cocoa_id};
#[cfg(all(feature = "desktop_winit", target_os = "macos"))]
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
#[cfg(all(feature = "desktop_winit", target_os = "macos"))]
use metal::{Device, MTLPixelFormat, MetalLayer};
#[cfg(feature = "desktop_winit")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(feature = "desktop_winit")]
use skia_safe as sk;
#[cfg(all(feature = "desktop_winit", target_os = "macos"))]
use zeno_backend_impeller::MetalSceneRenderer;
#[cfg(feature = "desktop_winit")]
use zeno_backend_skia::render_scene_to_canvas;
#[cfg(feature = "desktop_winit")]
use winit::application::ApplicationHandler;
#[cfg(feature = "desktop_winit")]
use winit::dpi::LogicalSize;
#[cfg(feature = "desktop_winit")]
use winit::event::WindowEvent;
#[cfg(feature = "desktop_winit")]
use winit::event_loop::{ActiveEventLoop, ControlFlow};
#[cfg(feature = "desktop_winit")]
use winit::window::{Window, WindowId};

#[derive(Debug, Clone, PartialEq)]
pub struct DesktopWindowHandle {
    pub id: String,
    pub title: String,
    pub size: (u32, u32),
    pub surface: crate::NativeSurface,
}

impl DesktopShell {
    #[cfg(feature = "desktop_winit")]
    pub fn run_window(&self, config: &WindowConfig) -> Result<(), ZenoError> {
        self.run_backend_scene_window(
            config,
            Backend::Skia,
            Scene {
                size: config.size,
                commands: vec![DrawCommand::Clear(zeno_core::Color::WHITE)],
            },
        )
    }

    #[cfg(feature = "desktop_winit")]
    pub fn run_scene_window(&self, config: &WindowConfig, scene: Scene) -> Result<(), ZenoError> {
        self.run_backend_scene_window(config, Backend::Skia, scene)
    }

    #[cfg(feature = "desktop_winit")]
    pub fn run_backend_scene_window(
        &self,
        config: &WindowConfig,
        backend: Backend,
        scene: Scene,
    ) -> Result<(), ZenoError> {
        use winit::event_loop::EventLoop;

        let event_loop = EventLoop::new()
            .map_err(|error| ZenoError::InvalidConfiguration(error.to_string()))?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut app = DesktopWindowApp {
            config: config.clone(),
            backend,
            scene,
            renderer: None,
            window_id: None,
            creation_error: None,
        };
        event_loop
            .run_app(&mut app)
            .map_err(|error| ZenoError::InvalidConfiguration(error.to_string()))?;
        if let Some(error) = app.creation_error {
            return Err(ZenoError::InvalidConfiguration(error));
        }
        Ok(())
    }

    #[cfg(not(feature = "desktop_winit"))]
    pub fn run_window(&self, _config: &WindowConfig) -> Result<(), ZenoError> {
        Err(ZenoError::InvalidConfiguration(
            "desktop_winit feature is disabled".to_string(),
        ))
    }
}

#[cfg(feature = "desktop_winit")]
struct DesktopWindowApp {
    config: WindowConfig,
    backend: Backend,
    scene: Scene,
    renderer: Option<DesktopGpuPresenter>,
    window_id: Option<WindowId>,
    creation_error: Option<String>,
}

#[cfg(feature = "desktop_winit")]
impl ApplicationHandler for DesktopWindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }

        match DesktopGpuPresenter::new(event_loop, self.backend, &self.config) {
            Ok(renderer) => {
                self.window_id = Some(renderer.window().id());
                renderer.window().request_redraw();
                self.renderer = Some(renderer);
            }
            Err(error) => {
                self.creation_error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window_id != Some(window_id) {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.draw_scene() {
                    self.creation_error = Some(error.to_string());
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    if let Err(error) = renderer.resize(size.width, size.height) {
                        self.creation_error = Some(error.to_string());
                        event_loop.exit();
                    } else {
                        renderer.window().request_redraw();
                    }
                }
            }
            WindowEvent::Destroyed => {
                self.renderer = None;
                self.window_id = None;
                event_loop.exit();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.window().request_redraw();
        }
    }
}

#[cfg(feature = "desktop_winit")]
impl DesktopWindowApp {
    fn draw_scene(&mut self) -> Result<(), ZenoError> {
        let renderer = self
            .renderer
            .as_mut()
            .ok_or_else(|| ZenoError::InvalidConfiguration("gpu renderer is not available".to_string()))?;
        renderer.draw_scene(&self.scene)
    }
}

#[cfg(feature = "desktop_winit")]
struct SkiaGlRenderer {
    window: Rc<Window>,
    gl_config: Config,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    gr_context: sk::gpu::DirectContext,
}

#[cfg(feature = "desktop_winit")]
enum DesktopGpuPresenter {
    Skia(SkiaGlRenderer),
    #[cfg(target_os = "macos")]
    Impeller(ImpellerMetalPresenter),
}

#[cfg(feature = "desktop_winit")]
impl DesktopGpuPresenter {
    fn new(
        event_loop: &ActiveEventLoop,
        backend: Backend,
        config: &WindowConfig,
    ) -> Result<Self, String> {
        match backend {
            Backend::Skia => SkiaGlRenderer::new(event_loop, config).map(Self::Skia),
            #[cfg(target_os = "macos")]
            Backend::Impeller => ImpellerMetalPresenter::new(event_loop, config).map(Self::Impeller),
            #[cfg(not(target_os = "macos"))]
            Backend::Impeller => Err("impeller desktop presenter is not implemented for this platform".to_string()),
        }
    }

    fn window(&self) -> &Window {
        match self {
            Self::Skia(renderer) => renderer.window.as_ref(),
            #[cfg(target_os = "macos")]
            Self::Impeller(renderer) => renderer.window.as_ref(),
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), ZenoError> {
        match self {
            Self::Skia(renderer) => renderer.resize(width, height),
            #[cfg(target_os = "macos")]
            Self::Impeller(renderer) => renderer.resize(width, height),
        }
    }

    fn draw_scene(&mut self, scene: &Scene) -> Result<(), ZenoError> {
        match self {
            Self::Skia(renderer) => renderer.draw_scene(scene),
            #[cfg(target_os = "macos")]
            Self::Impeller(renderer) => renderer.draw_scene(scene),
        }
    }
}

#[cfg(feature = "desktop_winit")]
impl SkiaGlRenderer {
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

        Ok(Self {
            window,
            gl_config,
            gl_context,
            gl_surface,
            gr_context,
        })
    }

    fn resize(&self, width: u32, height: u32) -> Result<(), ZenoError> {
        let width = NonZeroU32::new(width.max(1))
            .ok_or_else(|| ZenoError::InvalidConfiguration("invalid window width".to_string()))?;
        let height = NonZeroU32::new(height.max(1))
            .ok_or_else(|| ZenoError::InvalidConfiguration("invalid window height".to_string()))?;
        self.gl_surface.resize(&self.gl_context, width, height);
        Ok(())
    }

    fn draw_scene(&mut self, scene: &Scene) -> Result<(), ZenoError> {
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
        .ok_or_else(|| ZenoError::InvalidConfiguration("failed to wrap GL render target".to_string()))?;

        render_scene_to_canvas(surface.canvas(), scene);
        self.gr_context.flush_and_submit();
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .map_err(|error| ZenoError::InvalidConfiguration(error.to_string()))?;
        Ok(())
    }
}

#[cfg(all(feature = "desktop_winit", target_os = "macos"))]
struct ImpellerMetalPresenter {
    window: Rc<Window>,
    layer: MetalLayer,
    renderer: MetalSceneRenderer,
}

#[cfg(all(feature = "desktop_winit", target_os = "macos"))]
impl ImpellerMetalPresenter {
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
        let renderer =
            MetalSceneRenderer::new(device.clone(), queue.clone()).map_err(|error| error.to_string())?;

        Ok(Self {
            window,
            layer,
            renderer,
        })
    }

    fn resize(&self, width: u32, height: u32) -> Result<(), ZenoError> {
        self.layer
            .set_drawable_size(CGSize::new(width.max(1) as f64, height.max(1) as f64));
        Ok(())
    }

    fn draw_scene(&mut self, scene: &Scene) -> Result<(), ZenoError> {
        let drawable = self.layer.next_drawable().ok_or_else(|| {
            ZenoError::InvalidConfiguration("metal layer did not provide a drawable".to_string())
        })?;
        self.renderer.render_to_drawable(drawable, scene)
    }
}

#[cfg(all(feature = "desktop_winit", target_os = "macos"))]
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
    let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
        raw_window_handle,
        width,
        height,
    );
    let surface = unsafe { gl_display.create_window_surface(gl_config, &attrs) }
        .map_err(|error| error.to_string())?;
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
