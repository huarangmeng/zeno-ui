# Platform Shell

## Responsibility
- Create and describe native render surfaces.
- Hold platform metadata that informs backend preference.
- Keep platform-specific window/view creation out of the renderer abstraction.

## Current Platform Model
- Windows: shell is expected to bridge Win32-level surfaces and prefers Skia fallback first.
- macOS: shell is expected to bridge `NSView` and Metal-backed surfaces, so Impeller-style rendering is preferred.
- Linux: shell is expected to bridge Wayland/X11 surfaces and defaults to Skia first.
- Android: shell is expected to bridge `Surface` or `ANativeWindow`, with native renderer handoff preferred.
- iOS: shell is expected to bridge `UIView` and Metal layer surfaces, with Impeller-style rendering preferred.

## Current Implementation
- `MinimalShell` still exists as the cross-platform fallback that only produces a native surface descriptor.
- `DesktopShell` uses `winit` on Windows/macOS/Linux to create a real desktop window handle and derive surface metadata from it；`winit` 是 `zeno-shell` 的可选特性（desktop_winit），默认不启用，只在 demo 或宿主侧适配时开启。
- The current desktop path is intentionally thin: it validates window creation and platform identity without coupling the shell to a concrete render backend.

## Near-Term Evolution
- Replace desktop-only `winit` coverage with deeper per-platform window and view integrations where native handles matter.
- Introduce event loop and input dispatch contracts on top of the surface abstraction.
- Add platform capability reporting so runtime can distinguish build-time support from runtime availability.
