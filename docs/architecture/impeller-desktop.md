# Impeller Desktop

## Current State
- `zeno-runtime` still resolves desktop rendering to Skia on macOS because the Impeller scene renderer is not finished.
- `zeno-shell` now contains a backend-routed desktop presenter entry, so Skia and Impeller can share the same host-side window lifecycle.
- A macOS-only `ImpellerMetalPresenter` scaffold exists to reserve the Metal-specific integration point without changing the higher-level compose, graphics, or runtime APIs.

## Responsibility Split
- `zeno-shell` owns the native window, event loop, and GPU presenter bootstrap.
- `zeno-backend-impeller` should own scene-to-Impeller command translation once implemented.
- `zeno-runtime` should only decide whether Impeller is truly available on the current platform.

## Next Implementation Steps
- Bind the macOS presenter to a real Metal layer or swapchain target.
- Add scene submission from `zeno-backend-impeller` into the presenter instead of returning a scaffold error.
- Change the macOS runtime probe from “scaffold exists” to “available” only after the scene path is proven.
