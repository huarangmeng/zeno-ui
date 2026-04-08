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

## Near-Term Evolution
- Replace `MinimalShell` with real platform crates or per-platform modules that can create actual windows/views.
- Introduce event loop and input dispatch contracts on top of the surface abstraction.
- Add platform capability reporting so runtime can distinguish build-time support from runtime availability.
