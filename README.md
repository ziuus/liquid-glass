<div align="center">

# 🌊 Liquid Glass

### Physical Optical Lensing & Glassmorphism UI Engine for Linux & Wayland

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![WGPU](https://img.shields.io/badge/WGPU-0.19-blue?style=flat-square&logo=webgpu)](https://wgpu.rs)
[![Wayland](https://img.shields.io/badge/Wayland-Layer--Shell-2B3A42?style=flat-square&logo=linux)](https://wayland.freedesktop.org)
[![WGSL](https://img.shields.io/badge/Shader-WGSL-694382?style=flat-square)](https://www.w3.org/TR/WGSL/)
[![License](https://img.shields.io/badge/License-MIT-black?style=flat-square)](LICENSE)

<br />

<p align="center">
  <img src="assets/hero.png" alt="Liquid Glass Floating Media Island" width="620" style="border-radius: 16px; box-shadow: 0 20px 50px rgba(0,0,0,0.5);" />
</p>

*Physical plano-convex Snell refraction, Cauchy chromatic dispersion (RGB edge separation), directional meniscus glints, and tactile spring kinetics running natively on Wayland.*

<br />

---

</div>

## 🌟 Why Liquid Glass?

Most "glassmorphism" effects in modern web and desktop apps are superficial: they simply stack a flat 2D box-blur over a dimmed background. 

**Real optical glass interacts dynamically with light and geometry.** Liquid Glass models real physical optics directly in custom WGSL shaders:

| Optical Property | Standard Glassmorphism | Liquid Glass Engine |
| :--- | :--- | :--- |
| **Refraction** | ❌ None (Flat blur) | ✅ **Snell's Law ray-bending** through plano-convex lens domes |
| **Dispersion** | ❌ None | ✅ **Cauchy chromatic aberration** (RGB wavelength splitting at bevels) |
| **Fresnel Response** | ❌ Constant opacity | ✅ **Schlick's approximation** (grazing angles gleam with total internal reflection) |
| **Meniscus Edges** | ❌ Simple border stroke | ✅ **Directional sunlight highlights** + polished 1.2px rim glints |
| **Idle Overhead** | ⚠️ Constant redraws / CPU burn | ✅ **0.0% CPU Idle** via event-driven kernel `epoll_wait` |
| **Fragment Cost** | ⚠️ Fullscreen canvas redraws | ✅ **Tight bounded quads** — zero background overdraw |

<br />

## 📦 Architecture

The workspace is organized into modular, purpose-built crates:

```
liquid-glass/
├── glass-core/        # Hardware-accelerated rendering engine (WGPU + WGSL)
│   ├── renderer/      # Bounded pipeline, dynamic uniform offsets, optical shaders
│   ├── material/      # Physical material profiles (LiquidGlass, Frosted, Acrylic)
│   ├── animation/     # Sub-stepped Runge-Kutta 4 (RK4) spring physics
│   └── geometry/      # Procedural shapes (Capsule, RoundedRect, Circle)
├── glass-ui/          # High-level declarative UI components
│   └── GlassWidget, GlassCard, GlassButton, GlassToggle, GlassSlider
└── examples/
    ├── glass-panel/   # Wayland layer-shell floating media control island
    └── simple-window/ # Cross-platform desktop window playground (winit + wgpu)
```

<br />

## ⚡ Quickstart

Add `glass-core` and `glass-ui` to your `Cargo.toml`:

```toml
[dependencies]
glass-core = { git = "https://github.com/ziuus/liquid-glass" }
glass-ui   = { git = "https://github.com/ziuus/liquid-glass" }
```

### Building a Tactile Glass Interface

```rust
use glass_ui::{GlassWidget, GlassCard, GlassButton, GlassToggle, GlassSlider};
use glass_core::geometry::Shape;

let mut widget = GlassWidget::new();

// 1. Floating Glass Container Island
widget.add_card(
    GlassCard::new(0.1, 0.1, 0.8, 0.6)
        .shape(Shape::Capsule)
        .blur(24.0)
        .refraction(0.55)
        .dispersion(0.02)
        .bevel(8.0)
);

// 2. Interactive Tactile Controls
widget.add_button(GlassButton::new(0.20, 0.30, 0.25, 0.12, "Play"));
widget.add_toggle(GlassToggle::new(0.55, 0.30, 0.15, 0.08, true));
widget.add_slider(GlassSlider::new(0.20, 0.50, 0.60, 0.05, 0.45));

// 3. Event Loop Interaction & Single-Pass Render
widget.on_pointer_move(cursor_x, cursor_y);
widget.update(delta_time);

let surfaces = widget.collect_surfaces();
renderer.render(&surfaces)?;
```

<br />

## 🚀 Running the Demos

### 1. Floating Desktop Media Island (Wayland)
Spawns a floating desktop island on Hyprland, Sway, or any `wlr-layer-shell` compositor:

```bash
cargo run --release -p glass-panel
```

*Features rotating vinyl grooves, audio wave visualizers, interactive timeline scrubbing, play/pause toggles, and cursor-proximity edge flares.*

### 2. Standalone Window Playground
Cross-platform desktop window running on standard WGPU:

```bash
cargo run --release -p simple-window
```

<br />

## 🔬 Under the Hood

### 1. Plano-Convex Lens Refraction & Dispersion
Surface normals $N$ are computed procedurally in screen-space from heightfield gradients:

$$N = \text{normalize}\begin{pmatrix} -\nabla h_x \\ -\nabla h_y \\ 1 \end{pmatrix}$$

Refraction rays bend the background texture coordinates across separate color wavelengths (red, green, blue) according to Cauchy's dispersion formula:

$$\vec{uv}_{\text{color}} = \vec{uv} + (N_{xy} \cdot \eta_{\text{base}}) \pm \vec{\Delta}_{\text{dispersion}}$$

### 2. Zero-Overdraw Bounded Quads
Rather than blitting full-screen post-processing passes over the desktop, `GlassRenderer` rasterizes **tight clipped bounding quads** fitted to each individual glass surface with dynamic uniform buffer offsets (`has_dynamic_offset: true`). Fragment shaders execute exclusively inside the bounding footprint of the glass lens.

### 3. Sub-Stepped Spring Physics
Buttons and sliders use Runge-Kutta 4 (RK4) integration with configurable damping and stiffness presets (`snappy`, `smooth`, `bouncy`), delivering tactile compression and fluid recovery upon click or release.

<br />

## 🤝 Contributing

Contributions, bug reports, and component proposals are warmly welcome! 
Check out [CONTRIBUTING.md](CONTRIBUTING.md) to get started with development and architectural guidelines.

<br />

## 📄 License

Licensed under the [MIT License](LICENSE).
