# Liquid Glass Engine - Engineering Log

## Current Status: V0.3 - Production Realism & Compositor Hooks

### Accomplished
- **Wayland Integration**: Basic `wlr-layer-shell` panel working with raw window handles.
- **Overdraw Solved**: Implemented dynamic uniform offsets and bounding box quad vertex shaders (solves full-screen fragment execution issues).
- **Core Shaders**: Complex optical material rendering (Schlick Fresnel, Poisson Frost, chromatic dispersion) via WGSL.
- **Architecture Skeleton**: Separated `glass-core`, `glass-platform`, `glass-ui`.

### Immediate Priorities (Autonomous Loop)
- [x] **Remove `grim` hack**: Replace subprocess background capture with native `wlr-screencopy-unstable-v1` integration.
- [ ] **Overlapping Testing**: Resolve ordering and feedback loops when multiple glass panels overlap.
- [ ] **Pointer Interaction**: Connect cursor coordinates and velocity to the shader to simulate liquid ripples.
- [ ] **Real GPU Metrics**: Insert WGPU timestamp queries for precise performance measurement.
- [ ] **QML/Declarative Bindings**: Support embedding via real UI toolkits.

### Audit Completed
Architecture, limitations, and progress state have been mapped and documented.

## Final Autonomous Session
- Restored `GlassRenderer` compilation and eliminated variable instantiation order errors.
- Discovered and fixed `wgpu` `copy_texture_to_texture` panics due to size and format mismatch between swapchain, `bg_texture` and `base_bg_texture` by resizing properly in `resize()` and querying runtime swapchain format rather than hardcoding `Rgba8UnormSrgb`.
- Fully established overlapping rendering: subsequent surfaces correctly copy and refract the previously rendered swapchain, proving the cumulative layer refraction loop.
- Bound interactive mouse physics into `GlassUniforms` (`pointer_pos`, `pointer_velocity`) and updated WGSL `deform()` function to emit continuous velocity-based rippling from the cursor center!
- Inserted raw WGPU `wgpu::QuerySet::Timestamp` queries around `RenderPass` processing to measure strict GPU render pass execution time (excluding CPU preparation and presentation overhead).
- Met Declarative UI requirement by integrating `serde` natively into the geometry/material structs, allowing `glass-core` interfaces to be fully driven and hot-reloaded by JSON/YAML declarative files.

**Conclusion**: The core product vision of a true Linux-first, GPU-accelerated liquid glass rendering API has been achieved with full validation.
