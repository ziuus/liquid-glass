# Liquid Glass Engine Architecture

## 1. Overview
The Liquid Glass Engine is a Linux-first GPU-accelerated rendering and material framework for creating convincing liquid glass UI surfaces. It avoids standard glassmorphism tricks by integrating real physical optical models.

## 2. Component Structure
- `glass-core/`: Contains the wgpu-based rendering loop, material descriptors, WGSL shader compilation, uniform management, and optical processing. It knows NOTHING about Wayland, X11, or specific toolkits.
- `glass-platform/`: Adapters and capability detection. Deals with compositor features (e.g., blur capabilities vs raw screen copy).
- `glass-ui/`: High-level UI framework pieces (`GlassPanel`, `GlassButton`, etc.) for composing views.

## 3. Rendering Model
- Render passes use **tightly bounded clipped quads** rather than full-screen overlays to eliminate overdraw on large displays with many surfaces.
- Dynamic Uniform Buffers (`has_dynamic_offset: true`) allow batch rendering of over 100 surfaces in a single pass without queueing issues.
- Backdrop capture currently feeds raw pixels into a `Texture2D` via `queue.write_texture`.

## 4. Platform Integration
- Integrates with Wayland compositors (Hyprland, Niri) via `wlr-layer-shell` (and SCTK).
- A WaylandWindow wrapping `wl_surface` provides raw window handles to `wgpu`.
