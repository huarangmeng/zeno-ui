use std::rc::Rc;

#[allow(deprecated)]
use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use metal::{Device, MTLPixelFormat, MetalLayer};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;
use zeno_backend_impeller::MetalSceneRenderer;
use zeno_core::{zeno_session_log, Backend, Color, Platform, Size, ZenoError, ZenoErrorCode};
use zeno_graphics::{FrameReport, RenderSurface, Scene, SceneSubmit};

use super::desktop_session_error;
use super::scene::{default_clear_color, ensure_clear_command, partial_scene_for_dirty_bounds, patch_stats};

pub(super) struct ImpellerMetalSession {
    window: Rc<Window>,
    layer: MetalLayer,
    renderer: MetalSceneRenderer,
    surface: RenderSurface,
    clear_color: Color,
    last_scene: Option<Scene>,
}

impl ImpellerMetalSession {
    pub(super) fn new(event_loop: &ActiveEventLoop, config: &zeno_core::WindowConfig) -> Result<Self, String> {
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
        self.layer
            .set_drawable_size(CGSize::new(width.max(1) as f64, height.max(1) as f64));
        self.surface.size = Size::new(width.max(1) as f32, height.max(1) as f32);
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
        let drawable = self.layer.next_drawable().ok_or_else(|| {
            desktop_session_error(
                ZenoErrorCode::SessionNextDrawableUnavailable,
                "next_drawable",
                "metal layer did not provide a drawable",
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
            backend = ?Backend::Impeller,
            mode = if dirty_bounds.is_some() { "patch" } else { "full" },
            surface = %self.surface.id,
            scale_factor = self.window.scale_factor(),
            clear = ?self.clear_color,
            ?dirty_bounds,
            "impeller macos scene submit"
        );
        if let Some(bounds) = dirty_bounds {
            let partial_scene = partial_scene_for_dirty_bounds(&scene, bounds);
            self.renderer
                .render_to_drawable_with_load(drawable, &partial_scene, true)?;
        } else {
            self.renderer.render_to_drawable(drawable, &scene)?;
        }
        let (patch_upserts, patch_removes) = patch_stats(submit);
        Ok(FrameReport {
            backend: Backend::Impeller,
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
        format!(
            "clear:{:?} scale:{:.2}",
            self.clear_color,
            self.window.scale_factor()
        )
    }
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
