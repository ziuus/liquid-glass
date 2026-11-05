# 📦 Packaging & Architecture: Separating Engine from Desktop Rices

This guide outlines how to separate the **Liquid Glass Core Engine** from user-facing **Desktop Widgets & Rices** into clean, independent repositories, and how developers consume Liquid Glass as a library or framework.

---

## 🏛️ 1. Repository Boundary

```
┌────────────────────────────────────────────────────────┐
│  liquid-glass (github.com/ziuus/liquid-glass)          │
│  The Physical Rendering & Optics Engine                │
├────────────────────────────────────────────────────────┤
│  • glass-core     Headless WGPU pipeline & WGSL optics │
│  • glass-ui       Declarative widgets (Card, Button...)│
│  • glass-platform Layer-shell bindings & Wallpaper crop│
│  • docs & tests   Architecture, math, benchmarks       │
└────────────────────────────────────────────────────────┘
                           ▲
                           │ Git Dependency / crates.io
                           │
┌────────────────────────────────────────────────────────┐
│  liquid-glass-rice (github.com/ziuus/liquid-glass-rice)│
│  User Desktop Suite & Hyprland / Wayland Experience    │
├────────────────────────────────────────────────────────┤
│  • glass-island   Dynamic island, media, notifications │
│  • glass-todo     6-task productivity & calendar glass │
│  • glass-clock    Swiss minimalist battery & clock     │
│  • keybindings    Hyprland IPC & systemd user services │
└────────────────────────────────────────────────────────┘
```

---

## 🚀 2. How to Consume Liquid Glass as a Framework

When creating a new rice or standalone widget in a separate repository, configure `Cargo.toml`:

```toml
[package]
name = "my-liquid-rice"
version = "0.1.0"
edition = "2021"

[dependencies]
# Point to the official engine repository (or crates.io once published)
glass-core = { git = "https://github.com/ziuus/liquid-glass.git" }
glass-ui = { git = "https://github.com/ziuus/liquid-glass.git" }
glass-platform = { git = "https://github.com/ziuus/liquid-glass.git" }

# For local development across sibling directories:
# glass-core = { path = "../liquid-glass/glass-core" }
# glass-ui = { path = "../liquid-glass/glass-ui" }
# glass-platform = { path = "../liquid-glass/glass-platform" }
```

---

## 🎨 3. Building Custom Components with Liquid Glass

Every liquid glass component is created declaratively using `GlassSurface` and `GlassRenderer`:

```rust
use glass_core::geometry::Shape;
use glass_core::{GlassRenderer, GlassSurface, Material};

// 1. Define physical optics and layout
let island = GlassSurface::new()
    .material(Material::LiquidGlass)
    .shape(Shape::Capsule)
    .bounds(0.35, 0.02, 0.30, 0.08) // Normalized [x, y, w, h]
    .curvature(0.06)
    .bevel(14.0)
    .refraction(0.75)               // Snell's law deflection
    .dispersion(0.025)              // Cauchy RGB splitting
    .blur(4.0)                      // Subtle interior frosted diffusion
    .specular(1.20)                 // Directional sunlight glint
    .fresnel(0.75);

// 2. Single-pass render
renderer.render(&[island])?;
```

---

## ⌨️ 4. Dynamic Island Shortcuts & Controls

The desktop rice includes native compositor hotkeys and command line control:

| Shortcut | Action | Description |
| :--- | :--- | :--- |
| `SUPER + I` | **Toggle Island** | Animates between compact pill and hidden |
| `SUPER + H` | **Hide Island** | Slides island off-screen |
| `SUPER + D` | **Quick Access** | Expands app launcher with key hints `1..5` |
| `SUPER + ALT + M` | **Music Player** | Expands media island with active audio visualizer |

**CLI Control**:
```bash
island-ctl toggle      # Toggle visibility
island-ctl music       # Expand music player
island-ctl apps        # Open quick access launcher
island-ctl hide        # Hide
```

---

## 🛠️ 5. Publishing to Crates.io (Roadmap)

To package Liquid Glass for the broader Rust ecosystem:
1. `cargo publish -p glass-core`
2. `cargo publish -p glass-ui`
3. `cargo publish -p glass-platform`
