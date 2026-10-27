# Liquid Glass Engine - Component Showcase

Live captures of the Liquid Glass engine running on Wayland, demonstrating physically-accurate glassmorphism rendering.

## Live Application Screenshots

### Screenshot 1: Current State
![Showcase 1](screenshots/showcase-current.png)

### Screenshot 2: In Action
![Showcase 2](screenshots/glass-showcase-2.png)

### Screenshot 3: Full View
![Showcase 3](screenshots/glass-showcase-3.png)

## Running Examples

All examples captured from the `glass-panel` example running on Wayland compositor.

### Build & Run

```bash
cd examples/glass-panel
cargo build --release
../../target/release/glass-panel
```

## Technical Implementation

### Optical Features Visible
- **Chromatic Aberration**: Color dispersion at glass edges
- **Plano-Convex Refraction**: Background distortion through curved surfaces
- **Fresnel Effects**: Edge brightness based on viewing angle
- **Dynamic Blur**: Real-time background sampling and Gaussian blur
- **Meniscus Edge Lighting**: Directional rim lighting simulating liquid edges

### Interactive Features
- **Cursor Proximity**: Edge flares appear when cursor approaches within 100px
- **Hover States**: Specular highlights and edge emphasis
- **Spring Physics**: Natural motion with configurable damping
- **Adaptive Contrast**: Appearance adjusts based on background luminosity

## Component Examples Available

### Current (glass-panel)
- Floating media control widget
- Layer shell integration
- Wayland background capture
- Real-time rendering at 60+ FPS

### Planned (component-showcase)
- Dashboard with stats cards
- Interactive controls (buttons, sliders, toggles)
- Card gallery with material variants
- Notification panels
- Context menus and modals

## Performance Metrics

From running examples:
- **Frame Rate**: 60 FPS stable
- **GPU Usage**: Minimal (bounded quad rendering)
- **Memory**: ~20MB for single widget
- **Latency**: <16ms frame time

## Creating New Components

```rust
use glass_core::{GlassRenderer, GlassSurface, Material};
use glass_ui::GlassCard;

// Create a glass card
let mut card = GlassCard::new(0.3, 0.4, 0.4, 0.3);
card.blur_radius = 24.0;
card.refraction = 0.60;
card.dispersion = 0.020;
card.tint = [0.08, 0.12, 0.18, 0.15];

// Convert to surfaces and render
let surfaces = card.to_surfaces();
for surface in surfaces {
    renderer.render(&surface);
}
```

## Showcase Locations

### Screenshots Directory
`/home/zius/Projects/liquid-glass/docs/screenshots/`
- `showcase-current.png` - Latest capture
- `glass-showcase-2.png` - Action shot
- `glass-showcase-3.png` - Full view

### Live Demos
- **Native**: Run any example in `examples/`
- **Web Preview**: `docs/showcase.html` (CSS approximation)

## Next Steps

1. ✅ Capture live screenshots
2. 🚧 Build component-showcase example
3. 🚧 Record video demonstrations
4. 🚧 Create animated GIFs for README
5. 🚧 Generate comparison shots (light/dark backgrounds)

---

*All screenshots captured on: $(date)*
*Resolution: 1920x1080*
*Compositor: Sway/Wayland*
