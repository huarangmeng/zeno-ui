# Desktop Rendering

## Current Desktop Path
- `zeno-shell` owns the native desktop window, event loop, and GPU context lifecycle.
- `zeno-runtime` resolves the preferred backend at runtime.
- `zeno-shell::DesktopShell::run_backend_scene_window` receives the resolved backend kind and routes the scene into a backend-specific desktop presenter.
- `zeno-backend-skia` now owns actual Skia command translation for desktop rendering.

## Backend Responsibility
- `zeno-shell` is no longer responsible for interpreting `DrawCommand`.
- `zeno-backend-skia` provides `render_scene_to_canvas`, which translates `Scene` into Skia canvas operations, including text, fill, stroke, and clear.
- Desktop host integration exists to bind a window framebuffer or GPU context to a concrete backend; it should not duplicate rendering logic.

## Current Status
- Skia desktop path is implemented through a GL-backed Skia surface and is exercised by `minimal_app`.
- Impeller desktop presenter is not implemented yet. The desktop presenter layer now fails explicitly for Impeller instead of silently pretending to support it.

## Next Step For Impeller
- Add a dedicated `ImpellerDesktopRenderer` that owns Metal-backed or Vulkan-backed desktop swapchain integration.
- Keep runtime resolution unchanged: once the desktop Impeller presenter becomes available, `DesktopShell` can dispatch to it without changing compose, graphics, or runtime APIs.
