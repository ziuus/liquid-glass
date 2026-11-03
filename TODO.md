# Liquid Glass — Roadmap & TODO

## Current Milestone: Engine Robustness & Visual Craft
- [x] Single-pass physical Snell's law refraction on curved meniscus quad
- [x] Clear refractive rim smoothly transitioning into silky frosted bulk interior
- [x] Anti-aliased subpixel vector typography, checkboxes, and icons via `UIFont`
- [x] Zero-warning Rust workspace & CI test suite cross-environment compatibility
- [ ] Subpixel spectral caustic dispersion & ray-traced ambient shadow refinement

## Upcoming Milestone: Dynamic Island for Linux
> **Note**: Scheduled immediately after liquid glass engine reaches production perfection. Do not implement yet.

- [ ] **Dynamic Island for Linux (Wayland Layer-Shell)**:
  - Floating top-centered adaptive optical capsule widget on Wayland layer-shell (`zwlr_layer_shell_v1`).
  - Morphing fluid states: Compact idle capsule, media player island, system alert banner, timer tracker.
  - Smooth RK4 spring-driven geometry expansion and fluid shape morphing.
  - Native MPRIS DBus integration for player controls (album art, seek bar, playback actions).
  - Full touch, pointer, and keyboard interactivity with zero compositor drag.
