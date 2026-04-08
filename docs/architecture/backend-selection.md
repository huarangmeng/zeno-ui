# Backend Selection

## Policy
- Default preference is `PreferImpeller`.
- If the current platform exposes a usable Impeller path, runtime picks Impeller.
- If the Impeller probe fails and fallback is allowed, runtime switches to Skia.
- If a backend is forced explicitly, runtime honors that override and returns a structured failure when unavailable.
- Root crate keeps backend switching capability available by default, but desktop window opening is moved behind demo/host-side opt-in features instead of becoming a mandatory library dependency.
- The Skia crate ships with a default stub renderer and an optional `real_skia` feature for bringing in `skia-safe` based raster rendering without forcing native Skia builds on every consumer.

## Platform Matrix
| Platform | Impeller Path | Skia Fallback Path | Current Status |
| --- | --- | --- | --- |
| Windows | Planned, not implemented | Yes | Skia selected today |
| macOS | Preferred via Metal-backed shell | Yes | Impeller selected when available |
| Linux | Planned, not implemented | Yes | Skia selected today |
| Android | Preferred via native surface handoff | Yes | Impeller selected when available |
| iOS | Preferred via Metal-backed shell | Yes | Impeller selected when available |

## Failure Categories
- `NotImplementedForPlatform`: the backend strategy exists conceptually but the platform implementation is not present yet.
- `MissingPlatformSurface`: the shell could not supply the native surface type required by the backend.
- `MissingGpuContext`: the GPU path exists but cannot initialize in the current runtime environment.
- `RuntimeProbeFailed`: an unexpected probe error occurred and is carried as a string payload.

## Testing Expectations
- Resolver tests must prove Impeller-first behavior on supported platforms.
- Resolver tests must prove Skia fallback behavior on unsupported platforms.
- Resolver tests must prove forced-backend failure behavior when fallback is disabled.
- Example runs should print the actual desktop window metadata and the selected backend so shell creation and runtime selection are both exercised together.
