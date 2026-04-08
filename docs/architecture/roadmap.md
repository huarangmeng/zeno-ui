# Roadmap

## Phase 1
- Stabilize the workspace split and compile all crates together.
- Keep the API focused on shell, runtime selection, scene submission, and text primitives.
- Use Skia as the always-available fallback path.

## Phase 2
- Replace placeholder backend implementations with real Skia and Impeller integrations.
- Add concrete platform shell implementations for desktop and mobile targets.
- Expand capability probing to include GPU API choice, surface class, and text shaping engine.

## Phase 3
- Introduce a retained scene graph and explicit composition pipeline.
- Add a declaration-oriented UI layer on top of the rendering core.
- Add widget, layout, focus, accessibility, and theme systems without coupling them to a concrete renderer.
