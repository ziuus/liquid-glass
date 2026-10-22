# 🚀 High-Impact Technical Showcase Post

**Target Platforms:** dev.to / Hashnode / Medium / Rust Community / Hacker News

---

# Liquid Glass: Physical Optical Lensing & Glassmorphism UI Engine for Linux in Rust (WGPU)

Most "frosted glass" or "glassmorphism" implementations on the web and Linux desktops boil down to a simple 2D box-blur shader layered over a background capture. 

While that looks pleasant, it isn’t how real glass behaves. Real optical glass has **curvature, refraction, chromatic dispersion (wavelength splitting), surface tension, and directional specular sheen**.

Over the last few weeks, I’ve been building **[Liquid Glass](https://github.com/ziuus/liquid-glass)**: an open-source rendering engine and UI framework written in **Rust, WGPU, and WGSL** that brings true physical optical glass simulation to Linux desktop apps and Wayland widgets.

![Liquid Glass Live Media Island Demo](https://raw.githubusercontent.com/ziuus/liquid-glass/master/assets/media_island.gif)

---

## 🔬 The Optical Physics Under the Hood

Instead of drawing flat quads with blurred backdrops, Liquid Glass computes analytical Signed Distance Fields (SDFs) per fragment:

### 1. Plano-Convex Snell Refraction
We compute the surface height $h(p)$ and its normal gradient $\nabla h(p)$ analytically across the geometry boundary:
$$N = \text{normalize}\begin{pmatrix} -\nabla h_x \\ -\nabla h_y \\ 1 \end{pmatrix}$$

Using Snell's Law ($IOR = 1.33 - 1.52$), light rays bend towards the optical center, producing true optical barrel distortion and magnification of underlying desktop wallpapers.

### 2. Spectral Chromatic Dispersion
Real glass splits white light into spectral wavelengths at curved meniscus boundaries. Liquid Glass offsets red, green, and blue texture sample coordinates proportionally to the surface refraction angle:
```wgsl
let r_offset = refract_vec * (1.0 - dispersion);
let g_offset = refract_vec;
let b_offset = refract_vec * (1.0 + dispersion);
```
This gives chamfered edges and button rims an unmistakable physical glow.

### 3. Directional Solar Rim Lighting & Specular Sheen
A dual-lobe Blinn-Phong specular reflection catches a simulated directional sunlight vector (`[-0.6, -0.8]`), producing bright 1.5px grazing meniscus catches.

---

## ⚡ Performance: 0.0% CPU When Idle

A common pitfall with custom Linux widgets and shaders is battery drain from uncapped render loops. Liquid Glass was architected for zero idle overhead:

1. **Kernel `epoll_wait` Blocking**: When no audio is playing and no pointer is moving, the event loop stops spinning and sleeps in Wayland's `event_queue.blocking_dispatch()`. Top benchmark:
   ```
   PID    USER  %CPU  %MEM  COMMAND
   241729 zius   0.0   1.2  glass-panel
   ```
2. **Single Quad Draw Call**: The entire UI (capsule glass, vinyl grooves, music notes, equalizer bars, timeline, scrubber knob, transport buttons, speaker) is rendered in **a single quad draw call** inside WGSL.
3. **In-Memory Pre-Scaled Wallpaper Cache**: Moving or dragging the widget samples wallpaper crops in sub-milliseconds without disk I/O.

---

## 🎨 Building UI Components with `glass-ui`

Building high-level interfaces shouldn't require writing raw WGSL shaders. The workspace includes `glass-ui`, providing pre-built interactive components:

- **`GlassCard` / `GlassIsland`**: Physical glass containers with adjustable corner radius, curvature, and bevel.
- **`GlassButton`**: Tactile spring-animated buttons with click compression (`0.94x`) and smooth recovery.
- **`GlassToggle`**: Pill switch with spring-animated knob transitions.
- **`GlassSlider`**: Draggable timeline scrubber with track rail, active fill, and thumb.

```rust
use glass_ui::{GlassWidget, GlassCard, GlassButton, GlassToggle, GlassSlider};

let mut widget = GlassWidget::new();

// Floating container
widget.add_card(GlassCard::island(0.1, 0.1, 0.8, 0.8).blur(32.0));

// Interactive controls
widget.add_button(GlassButton::new(0.15, 0.25, 0.30, 0.10, "Play"));
widget.add_toggle(GlassToggle::new(0.50, 0.25, 0.15, 0.08, true));
widget.add_slider(GlassSlider::new(0.15, 0.50, 0.70, 0.05, 0.65));

// In your event loop:
widget.on_pointer_move(x, y);
widget.update(dt);
let surfaces = widget.collect_surfaces();
renderer.render(&surfaces)?;
```

---

## 🚀 Two Running Targets

1. **Floating Wayland Desktop Island (`glass-panel`)**: Uses `zwlr_layer_shell_v1` on Hyprland/wlroots to float transparently over your wallpaper with real-time MPRIS media playback control.
   ```bash
   cargo run -p glass-panel --release
   ```
2. **Cross-Platform Desktop Window (`simple-window`)**: Uses standard `winit` and `wgpu` to render a windowed UI component playground.
   ```bash
   cargo run -p simple-window
   ```

---

## 🤝 Open Source & Contributing

We'd love community help expanding this into a full-featured desktop UI ecosystem:
- Building new `glass-ui` components (`GlassTextInput`, `GlassTooltip`, `GlassContextMenu`).
- Supporting more compositor protocols (KDE KWin blur, COSMIC compositor).
- New showcase widgets (weather, clock, system monitor).

- **GitHub Repository**: [https://github.com/ziuus/liquid-glass](https://github.com/ziuus/liquid-glass)
- **Contributing Guide**: [CONTRIBUTING.md](https://github.com/ziuus/liquid-glass/blob/master/CONTRIBUTING.md)

If you find this interesting or want to support native graphics on Linux, consider starring the repo on GitHub! ⭐
