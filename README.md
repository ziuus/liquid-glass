<div align="center">

# 🪟 Liquid Glass

**True Apple-Grade Physical Liquid Glass & Optical Lensing Engine for Wayland**

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org)
[![WGPU](https://img.shields.io/badge/WGPU-0.19-blue?style=for-the-badge&logo=webgpu)](https://wgpu.rs)
[![Wayland](https://img.shields.io/badge/Wayland-Layer--Shell-green?style=for-the-badge&logo=linux)](https://wayland.freedesktop.org)
[![WGSL](https://img.shields.io/badge/Shader-WGSL-purple?style=for-the-badge)](https://www.w3.org/TR/WGSL/)
[![License](https://img.shields.io/badge/License-MIT-black?style=for-the-badge)](LICENSE)

<br />

<p align="center">
  <img src="assets/preview.png" alt="Liquid Glass Control Center Widget" width="380" />
</p>

*Physical plano-convex refraction, chromatic dispersion, directional meniscus lighting, and second-order spring kinetics running natively at 60 FPS on Linux with 0% idle CPU footprint.*

</div>

---

## ✨ Features

- **💎 Genuine Optical Refraction**: Implements Snell's Law and planar refractive displacement using continuous WGSL Signed Distance Fields (SDF) and normal gradients.
- **🌈 Chromatic Dispersion**: Physically accurate spectral splitting ($RGB$ index-of-refraction variance) along curved edges and chamfered menisci.
- **☀️ Directional Illumination**: Eliminates artificial 360° white borders; uses genuine top-left solar lighting with delicate crest catches and edge ambient occlusion.
- **⚡ Reactive Spring Kinetics**: Second-order underdamped harmonic oscillators (`k=260`, `c=18`) with Gaussian proximity waves and anchor scaling.
- **🍃 Zero-Power Idle Loop**: Decouples from vblank frame callbacks once springs settle. Idle state consumes **0.0% CPU / GPU**, waking up instantly on cursor input.
- **🖥️ Native Wayland Integration**: Built directly atop `smithay-client-toolkit` and `wlr-layer-shell`, seamlessly rendering onto desktop wallpapers behind client windows or as floating widgets.
- **🎨 Refined Apple SF Glyphs**: Ultra-clean, delicate vector icons and straight frosted slider tracks rendered procedurally in shader space.

---

## 🔬 Optical Pipeline

```
       Incoming Light Ray
             │
             ▼
┌───────────────────────────────┐
│     Plano-Convex Chamfer      │ ──► Normal Map Gradient (∇h)
└────────────┬──────────────────┘
             ▼
┌───────────────────────────────┐
│     Snell Refraction (IOR)    │ ──► Barrel Distortion & Magnification
└────────────┬──────────────────┘
             ▼
┌───────────────────────────────┐
│     Spectral Dispersion       │ ──► Wavelength Separation (RGB offsets)
└────────────┬──────────────────┘
             ▼
┌───────────────────────────────┐
│     Dual Blinn-Phong Sheen    │ ──► Micro-Surface Specular + Spotlight
└────────────┬──────────────────┘
             ▼
┌───────────────────────────────┐
│     Fresnel Edge Catch        │ ──► Grazing-Angle Luminescence
└───────────────────────────────┘
```

---

## 🛠️ Architecture

| Crate | Responsibility |
|---|---|
| **`glass-core`** | WGPU rendering backend, WGSL optical shaders, pipeline caching, uniform bindings. |
| **`glass-platform`** | Wayland Layer Shell, X11/wlroots abstractions, native window handles. |
| **`glass-ui`** | Component abstractions, geometry descriptors, and material parameter definitions. |
| **`glass-panel`** | Example desktop Control Center widget with interactive sliders and capsules. |

---

## 🚀 Quickstart

### Prerequisites

Ensure you have the Vulkan loader and Wayland libraries installed:

```bash
# Arch Linux
sudo pacman -S rust vulkan-radeon vulkan-intel wayland grim

# Ubuntu / Debian
sudo apt install cargo libvulkan-dev libwayland-dev
```

### Build & Run

```bash
# Clone repository
git clone https://github.com/ziuus/liquid-glass.git
cd liquid-glass

# Run optimized release build
cargo run -p glass-panel --release
```

### Capture GPU Snapshot

To capture a lossless offscreen frame directly from the swapchain buffer:

```bash
GLASS_SNAPSHOT=snapshot.png cargo run -p glass-panel --release
```

---

## 🎛️ Material Configuration

Surfaces can be customized through the fluent builder API:

```rust
use glass_core::{GlassSurface, Material, geometry::Shape};

let panel = GlassSurface::new()
    .material(Material::LiquidGlass)
    .shape(Shape::RoundedRectangle { radius: 28.0 })
    .bounds(0.0, 0.0, 1.0, 1.0)
    .curvature(0.08)       // Optical dome curvature
    .bevel(10.0)           // Chamfer edge radius in px
    .refraction(0.24)      // Background displacement magnitude
    .dispersion(0.010)     // Chromatic aberration spread
    .blur(3.5)             // Creamy frosted backdrop blur radius
    .specular(0.25)        // Ambient Blinn-Phong reflection
    .fresnel(0.40);        // Grazing angle reflection
```

---

## 📜 License

Distributed under the **MIT License**. See [`LICENSE`](LICENSE) for details.
