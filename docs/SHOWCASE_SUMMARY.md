# Liquid Glass Engine - Live Showcase

✅ **CAPTURED: Live screenshots of the glass-panel example running on Wayland**

## What We Have

### 1. Live Application Running
- Built and ran `examples/glass-panel` successfully
- Captured 3 screenshots at different moments
- All saved to `docs/screenshots/`

### 2. Screenshots Captured (1920x1080)
```
docs/screenshots/
├── showcase-current.png    (873 KB) - Initial state
├── glass-showcase-2.png    (743 KB) - Mid-action
└── glass-showcase-3.png    (1.2 MB) - Full view
```

### 3. Documentation Created
- ✅ `docs/COMPONENT_SHOWCASE.md` - Showcase guide with embedded screenshots
- ✅ `docs/showcase.html` - Interactive web demo (CSS approximation)
- ✅ `examples/component-showcase/` - New Rust example with multiple scenes

## What's Actually Running

The **glass-panel** example demonstrates:
- Floating glass media control widget (like in your original screenshot)
- Real-time Wayland background capture
- Physical optical rendering (chromatic aberration, refraction, blur)
- Layer shell integration for overlay rendering

### Technical Stack
- **Renderer**: WebGPU (wgpu 0.19)
- **Shaders**: WGSL with physical optics
- **Platform**: Wayland/Sway via smithay-client-toolkit
- **Physics**: Spring animation system
- **Performance**: 60+ FPS with bounded quads

## How to Showcase

### View Screenshots
```bash
cd /home/zius/Projects/liquid-glass
xdg-open docs/screenshots/showcase-current.png
```

### Run Live Demo
```bash
cd examples/glass-panel
cargo run --release
```

### View Web Preview
```bash
cd docs
python3 -m http.server 8080
# Open http://localhost:8080/showcase.html
```

## Component Architecture

### glass-core (Engine)
```
renderer/     - WebGPU pipeline, shader compilation
material/     - Material presets (vibrancy, frost, liquid)
geometry/     - Shape primitives, surface batching
animation/    - Spring physics system
optical/      - Refraction, dispersion, Fresnel
```

### glass-ui (Components)
```rust
GlassCard     - Container panels
GlassButton   - Interactive buttons
GlassSlider   - Value controls
GlassToggle   - Binary switches
```

### glass-platform (Integration)
```
Wayland backend
X11 backend (planned)
Compositor capability detection
```

## Next Steps for More Showcasing

### 1. More Example Variations
```bash
cd examples/component-showcase
cargo build --release
# Scenes: Dashboard, Controls, Cards, Notifications, Media
```

### 2. Video Capture
```bash
wf-recorder -f showcase-demo.mp4 -t 30
```

### 3. Animated GIFs
```bash
ffmpeg -i showcase-demo.mp4 -vf "fps=30,scale=800:-1" showcase.gif
```

### 4. Comparison Shots
Run examples against different backgrounds:
- Dark wallpapers
- Light wallpapers
- Colorful gradients
- Desktop with windows behind

## What Makes This Special

Unlike CSS `backdrop-filter` approximations, this engine provides:

1. **Physical Chromatic Aberration**: RGB channel separation based on Abbe number
2. **Plano-Convex Refraction**: Real Snell's law light bending
3. **Meniscus Edge Effects**: Liquid-like rim lighting
4. **Dynamic Blur**: Real-time background sampling (not pre-composited)
5. **Bounded Rendering**: No full-screen fragment overdraw
6. **Fresnel Response**: View-angle dependent edge brightness

## Files Generated

### Source Code
- `examples/component-showcase/src/main.rs` - Wayland app framework
- `examples/component-showcase/src/components.rs` - 5 scene generators
- `examples/component-showcase/Cargo.toml` - Dependencies
- `examples/component-showcase/README.md` - Usage guide

### Documentation
- `docs/COMPONENT_SHOWCASE.md` - This showcase guide
- `docs/showcase.html` - Interactive web preview
- `docs/screenshots/` - Live captures directory

### Updated
- `Cargo.toml` - Added component-showcase to workspace members

---

**Status**: ✅ Working showcase with live screenshots captured
**Next**: Build component-showcase for more variety, record videos
