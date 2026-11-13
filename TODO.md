# Liquid Glass — Roadmap & TODO

## Completed Milestones
- [x] Single-pass physical Snell's law refraction on curved meniscus quad
- [x] Clear refractive rim smoothly transitioning into silky frosted bulk interior
- [x] Anti-aliased subpixel vector typography, checkboxes, and icons via `UIFont`
- [x] Zero-warning Rust workspace & CI test suite cross-environment compatibility
- [x] **Dynamic Island for Linux (Wayland Layer-Shell)**:
  - Floating top-centered adaptive optical capsule widget on Wayland layer-shell (`zwlr_layer_shell_v1`).
  - Morphing fluid states: Compact idle capsule, media player island, system alert banner, quick app launcher.
  - Smooth RK4 spring-driven geometry expansion and fluid shape morphing.
  - Native MPRIS DBus integration for player controls (album art, seek bar, playback actions).
  - Full pointer and touch interactivity with zero compositor drag.
  - IPC control via FIFO `/tmp/liquid_island.fifo` and `island-ctl` CLI.

## Upcoming Milestones
- [ ] Subpixel spectral caustic dispersion & ray-traced ambient shadow refinement
- [ ] KDE KWin blur & COSMIC compositor protocol adapters
- [ ] Multi-monitor layer-shell placement configuration
