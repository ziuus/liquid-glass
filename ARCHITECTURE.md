# Liquid Glass Engine Architecture

## 1. Overview
The Liquid Glass Engine is a Linux-first, GPU-accelerated rendering and physical optics framework built in Rust (`wgpu` + WGSL). It models physical optical lensing directly on the GPU—substituting superficial 2D box-blurs with true Snell's Law refraction, Cauchy chromatic dispersion, directional specular meniscus glints, and tactile Runge-Kutta 4 (RK4) spring physics.

## 2. Workspace Crates & Structure

```
liquid-glass/
├── glass-core/        # Strictly headless GPU rendering engine (WGPU + WGSL)
│   ├── renderer/      # Bounded quad pipeline, dynamic uniform offsets, glass.wgsl shaders
│   ├── material/      # Physical material profiles (LiquidGlass, Frosted, Acrylic, Obsidian)
│   ├── animation/     # Sub-stepped Runge-Kutta 4 (RK4) spring kinetics
│   ├── geometry/      # Procedural SDF shapes (Capsule, RoundedRect, Notched docked island)
│   ├── font/          # Hardware/CPU vector glyph rasterizer & Swiss typography engine
│   └── tokens/        # Apple HIG & VisionOS physical geometry & motion design tokens
├── glass-platform/    # Compositor adapters, platform window wrappers, capability detection
│   └── wallpaper/     # Sub-millisecond recursive inotify wallpaper watcher (awww/swww) & WCAG luminance analyzer
├── glass-ui/          # High-level retained-mode UI widgets (GlassCard, GlassButton, GlassToggle, GlassSlider)
└── examples/
    ├── liquid-shell/  # Flagship desktop shell suite: Dynamic Island, Swiss Analog Dial, Digital Clock, Tasks panel
    ├── glass-panel/   # Standalone floating media control island
    ├── glass-clock/   # Standalone physical liquid glass clock widget
    ├── glass-todo/    # Standalone glass task management widget
    └── simple-window/ # Cross-platform desktop window playground (winit + wgpu)
```

## 3. Optical Shading Pipeline (`glass.wgsl`)

1. **Procedural Signed Distance Fields (SDF)**:
   - Capsules, Rounded Rectangles, and Top-Attached Fillet Notches (`rr_sdf_notched`).
   - Normals are evaluated analytically via forward-difference SDF gradient vectors ($\nabla \text{SDF}$).
2. **Snell's Law Refraction & Cauchy Dispersion**:
   - Outward meniscus vectors bend background ray UVs across 7 spectral wavelength taps:
     $$\vec{uv}_{\text{color}} = \vec{uv} + (\vec{N}_{xy} \cdot \eta_{\text{base}}) \pm \vec{\Delta}_{\text{dispersion}}$$
   - Uses a $C^1$ continuous zero-slope lens profile ($1 - \text{smoothstep}(0, 1, t)$) with flat derivatives at both ends to eliminate bezel boundary kinks.
3. **Directional Specular Meniscus Glint**:
   - Razor-thin 1.0px crystalline highlight strictly facing the top-left key light ($\vec{L} = [-0.45, -0.65]$). Zero artificial stroke or cartoon lines on shadow-facing edges.
4. **Directional Ambient Contact Shadow**:
   - Tight 7.0px contact shadow cast strictly towards bottom-right ($-\vec{L}$); zero shadow halo on lit edges.
   - Smooth cubic falloff $(1 - \text{smoothstep}(0, 1, d))^2$ blended cleanly into the underlying desktop wallpaper at 7% maximum opacity in light mode (3.5% in dark mode).

## 4. Single-Pass Bounded Quad Rendering

- **Zero Full-Screen Overdraw**: Rather than running full-screen canvas blits, `GlassRenderer` generates tight, clipped bounding quads dilated only by the optical refraction margin.
- **Dynamic Uniform Buffers**: Batches all active surfaces into a single draw loop using dynamic uniform offset alignment (`min_uniform_buffer_offset_alignment`).

## 5. Desktop Shell Integration (`liquid-shell`)

- **Wayland Layer Shell**: Integrates via `smithay-client-toolkit` across top and overlay layers with non-blocking kernel `epoll_wait` (0.0% CPU idle).
- **Sub-Millisecond Wallpaper Synchronization**: Recursive inotify watcher detects wallpaper swaps instantly (`awww` cache, `swww`, or filesystem) without costly shell polling.
- **Live Configuration Hot-Reload**: Watches `~/.config/liquid-glass/config.json` via inotify, dynamically reconfiguring edge styles, themes, and dimensions without restarting the process.
- **Non-blocking FIFO IPC**: Command pipe at `/tmp/liquid_island.fifo` exposes external script and keybinding control over slides, themes, and layouts.
