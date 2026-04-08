# Rendering Architecture

## Goals
- Keep Rust as the framework control plane.
- Prefer Impeller-style rendering paths where the platform can provide them.
- Fall back to Skia when Impeller is not implemented or not available.
- Expose a backend-agnostic rendering API to upper layers.

## Layering
- `zeno-core` owns shared types, configuration, platform identity, and structured errors.
- `zeno-text` owns text descriptors, layout contracts, and fallback text measurement.
- `zeno-graphics` owns the drawing model, scene representation, renderer trait, and backend probe contract.
- `zeno-runtime` owns backend ordering, probing, fallback selection, and initialization policy.
- `zeno-backend-impeller` and `zeno-backend-skia` implement the rendering backend contract.

## Rendering Flow
1. The shell resolves the current platform and creates a native surface descriptor.
2. The runtime reads `RendererConfig` and builds a backend resolution order.
3. Each backend probes the active platform for support and capabilities.
4. The first available backend creates a `Renderer`.
5. Upper layers submit a `Scene` built from backend-neutral drawing commands.
6. The selected renderer produces a `FrameReport` and exposes the final backend choice.

## Why This Shape
- The selector lives in runtime so the rest of the framework never hardcodes Skia or Impeller.
- The graphics API stays stable even when a platform-specific backend strategy changes.
- Text remains separate because UI frameworks need text early, but text implementation depth can evolve independently from shape rendering.
