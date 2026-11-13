# Liquid Glass – Usage Guide

## Overview

This guide shows how to use `glass-core` and `glass-ui` to build authentic liquid glass UI surfaces with physical refraction, spring physics, and proper layer compositing.

> **Note**: All coordinates and sizes passed to `GlassSurface` / `GlassCard` are **normalized** (0.0–1.0) relative to the output surface dimensions.

---

## Cargo.toml

```toml
[dependencies]
glass-core = { git = "https://github.com/ziuus/liquid-glass" }
glass-ui   = { git = "https://github.com/ziuus/liquid-glass" }
```

---

## Core Concepts

### GlassSurface (glass-core)

The fundamental unit — a single bounded glass lens. Build one with the fluent API:

```rust
use glass_core::{GlassSurface, Material};
use glass_core::geometry::Shape;

let surface = GlassSurface::new()
    .material(Material::LiquidGlass)
    .shape(Shape::Capsule)
    .bounds(0.1, 0.1, 0.8, 0.12)   // x, y, w, h (normalized 0–1)
    .curvature(14.0)                 // lens dome depth
    .bevel(16.0)                     // rim width for specular glint
    .refraction(16.0)                // pixel displacement strength
    .dispersion(0.12)                // chromatic RGB split at bevel
    .specular(1.15)                  // specular highlight multiplier
    .fresnel(0.60)                   // Schlick Fresnel grazing angle
    .blur(0.0)                       // optional frosted-glass blur radius
    .style_mode(2.0)                 // 0=clean, 1=frosted, 2=liquid glass
    .tint(0.0, 0.0, 1.0, 0.0)       // (unused, time, alpha, is_light)
    .deformation(0.0)                // 0=float, 1=edge-straight, 2=edge-curved
    .element_type(0.0)               // GPU-side element shader id (0 = none)
    .pointer_pos(cursor_x, cursor_y);
```

### Rendering (glass-core)

```rust
use glass_core::{GlassRenderer, SharedGpu};

// Initialize renderer (async)
let gpu = SharedGpu::new(&window, width, height).await?;
let mut renderer = GlassRenderer::new_with_gpu(gpu.clone(), width, height)?;

// Each frame:
let surfaces: Vec<GlassSurface> = collect_my_surfaces();
renderer.render(&surfaces)?;
```

---

## High-Level Widgets (glass-ui)

`glass-ui` wraps `GlassSurface` into convenient retained-mode components.

### GlassCard

A floating glass container:

```rust
use glass_ui::GlassCard;
use glass_core::geometry::Shape;
use glass_core::Material;

let card = GlassCard::new(0.1, 0.1, 0.8, 0.6)   // x, y, w, h
    .shape(Shape::Capsule)
    .material(Material::LiquidGlass)
    .blur(0.0)
    .refraction(16.0)
    .dispersion(0.12)
    .bevel(14.0)
    .specular(1.2);

// Convert to a GlassSurface for the renderer
let surface = card.to_surface();
```

### GlassButton

```rust
use glass_ui::GlassButton;

let mut btn = GlassButton::new(0.20, 0.30, 0.25, 0.12, "Play");

// Pointer events
btn.on_pointer_move(px, py);
if btn.on_pointer_down(px, py) {
    println!("pressed");
}
if btn.on_pointer_up(px, py) {
    println!("released / clicked");
}

// Animate spring physics
btn.update(delta_time);

// Get surface
let surface = btn.to_surface();
```

### GlassToggle

```rust
use glass_ui::GlassToggle;

let mut toggle = GlassToggle::new(0.55, 0.30, 0.15, 0.08, true);

toggle.on_pointer_down(px, py); // clicks toggle
toggle.update(delta_time);

let surfaces = toggle.to_surfaces(); // returns thumb + track
```

### GlassSlider

```rust
use glass_ui::GlassSlider;

let mut slider = GlassSlider::new(0.20, 0.50, 0.60, 0.05, 0.45); // initial value = 0.45

slider.on_pointer_down(px, py);
slider.on_pointer_move(px, py); // dragging
slider.on_pointer_up();
slider.update(delta_time);

let value = slider.value;      // current 0.0–1.0 value
let surfaces = slider.to_surfaces();
```

### GlassWidget (collection)

Manage multiple components together:

```rust
use glass_ui::{GlassWidget, GlassCard, GlassButton, GlassToggle, GlassSlider};
use glass_core::geometry::Shape;

let mut widget = GlassWidget::new();

widget.add_card(
    GlassCard::new(0.1, 0.1, 0.8, 0.6)
        .shape(Shape::RoundedRectangle { radius: 18.0 })
        .refraction(16.0)
        .dispersion(0.12)
        .bevel(14.0),
);
widget.add_button(GlassButton::new(0.20, 0.30, 0.25, 0.12, "Play"));
widget.add_toggle(GlassToggle::new(0.55, 0.30, 0.15, 0.08, true));
widget.add_slider(GlassSlider::new(0.20, 0.50, 0.60, 0.05, 0.45));

// Event loop
widget.on_pointer_move(cursor_x, cursor_y);
widget.update(delta_time);

// Collect all surfaces for a single render call
let surfaces = widget.collect_surfaces();
renderer.render(&surfaces)?;
```

---

## Spring Physics (glass-core)

All animations use sub-stepped RK4 spring physics.

### Single-axis Spring

```rust
use glass_core::{Spring, SpringParams};

let mut opacity = Spring::new(1.0);
opacity.params = SpringParams::smooth();

opacity.set_target(0.0); // fade out

// In render loop:
opacity.update(delta_time);
let alpha = opacity.value; // current animated value

// Check if settled
if opacity.is_at_rest() {
    // skip update
}
```

### Spring Presets

| Preset | Stiffness | Damping | Use case |
|--------|-----------|---------|----------|
| `default_ui()` | 300 | 0.8 | General UI |
| `bouncy()` | 200 | 0.6 | Playful interactions |
| `smooth()` | 300 | 0.85 | Gentle transitions |
| `snappy()` | 400 | 0.8 | Immediate response |
| `gentle()` | 180 | 0.9 | Large slow motions |

### Multi-axis Springs

```rust
use glass_core::{SpringVec2, SpringVec4, SpringParams};

// Position
let mut pos = SpringVec2::new([0.0, 0.0]);
pos.set_target([0.5, 0.3]);
pos.update(delta_time);
let [x, y] = pos.value();

// Color (RGBA)
let mut color = SpringVec4::with_params(
    [1.0, 1.0, 1.0, 1.0],
    SpringParams::smooth(),
);
color.set_target([0.5, 0.7, 1.0, 0.9]);
color.update(delta_time);
```

### GlassTransition (hover/press states)

```rust
use glass_core::GlassTransition;

let mut transition = GlassTransition::new();
let base_blur = 0.0f32;

// On hover enter
transition.hover_in(base_blur);  // sharpens blur, lifts scale to 1.02

// On hover exit
transition.hover_out(base_blur);

// On press
transition.press(base_blur);     // scale down to 0.97, slightly more blur

// On release
transition.release(base_blur);

// Each frame
transition.update(delta_time);

// Use animated values
let blur  = transition.blur_radius.value;
let scale = transition.scale.value;
let alpha = transition.opacity.value;
```

---

## Material Presets (glass-core)

`GlassMaterial` stores vibrancy, blur, and shadow parameters. These are used by the material preset system and serialization but do **not** directly drive `GlassRenderer` (which uses `GlassSurface`). They are useful for configuring visual parameters and passing them into surfaces.

```rust
use glass_core::{GlassMaterial, MaterialType};

// Sidebar — heavy blur, high saturation
let sidebar = GlassMaterial::sidebar();

// Menu — medium frosted glass
let menu = GlassMaterial::menu();

// Popover — sheet glass with deep shadow
let popover = GlassMaterial::popover();

// Custom
let custom = GlassMaterial::new(MaterialType::HUD)
    .with_dark_mode()
    .with_corner_radius(10.0)
    .with_shadow(40.0, [0.0, 8.0], 0.3);

// Apply reduced transparency (accessibility)
let accessible = GlassMaterial::menu().with_reduced_transparency();
```

---

## JSON Serialization

`GlassSurface` is fully `serde`-serializable for config-driven hot-reloading:

```rust
use glass_core::geometry::{load_from_json, save_to_json};

// Save
let json = save_to_json(&surfaces)?;
std::fs::write("surfaces.json", &json)?;

// Load
let json = std::fs::read_to_string("surfaces.json")?;
let surfaces = load_from_json(&json)?;
```

---

## Performance Tips

1. **Zero overdraw**: Each surface renders only within its bounding quad — do not use full-screen passes.
2. **Idle CPU = 0%**: The renderer uses `epoll_wait`; call `render()` only when state changes.
3. **Skip at rest**: Check `spring.is_at_rest()` before calling `update()`.
4. **Batch surfaces**: Pass all surfaces in a single `renderer.render(&surfaces)` call.
5. **Blur sparingly**: `blur_radius` > 0 triggers a multi-tap frost sample. Use `0.0` for maximum performance.

---

## Troubleshooting

**Glass is invisible / black**
- Ensure `style_mode(2.0)` is set (Liquid Glass mode)
- Verify bounds are in 0.0–1.0 range
- Check `bevel` > 0 and `curvature` > 0

**Refraction looks wrong**
- Increase `refraction()` strength (try 12–20)
- Check `dispersion()` is 0.08–0.14 for visible chromatic split

**Animations feel stiff**
- Lower damping: `SpringParams { damping: 0.6, .. }` for more bounce
- Increase stiffness for faster response

**Wayland surface not appearing**
- Requires a compositor with `wlr-layer-shell-unstable-v1` support (Hyprland, Sway, niri, etc.)
- Run `cargo run -p glass-panel` to test layer shell

---

## Next Steps

- Flagship desktop shell: `examples/liquid-shell/` — run `./install.sh`
- Floating media island: `cargo run --release -p glass-panel`
- Cross-platform window: `cargo run --release -p simple-window`
- Render snapshot tests: `cargo test -p glass-core --test test_render_shader`
- Architecture: [`ARCHITECTURE.md`](../ARCHITECTURE.md)
- Configuration & IPC: [`README.md`](../README.md)
