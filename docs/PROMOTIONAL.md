# Liquid Glass - Promotional Materials

## Elevator Pitch (30 seconds)

> **Liquid Glass** is a physically-accurate glassmorphism rendering engine for Linux. Unlike CSS `backdrop-filter`, it simulates real optical physics: chromatic aberration, Snell refraction, Fresnel effects, and meniscus edges. Built with Rust and WebGPU for 60+ FPS performance on Wayland.

## Social Media Posts

### Twitter/X Thread

```
🌊 Introducing Liquid Glass - Physical glass rendering for Linux

Most glassmorphism is fake. CSS just blurs and adds opacity.

Liquid Glass simulates REAL physics:
• Chromatic aberration (RGB channel separation)
• Snell refraction (light bending)
• Fresnel effects (angle-dependent edges)
• Meniscus lighting (liquid-like rims)

Built with Rust 🦀 + WebGPU
60+ FPS on Wayland

[Screenshot]

---

Why does this matter?

Real physical simulation creates depth and realism that simple blur can't match.

Watch how light splits into RGB at glass edges (chromatic aberration):

[Zoomed screenshot showing color separation]

---

The tech stack:
• WGSL shaders for optical calculations
• Spring physics for natural motion
• Wayland layer shell for overlays
• Bounded rendering (no full-screen overdraw)

Perfect for:
• Desktop widgets
• System UI overlays
• Media controls
• Notification panels

---

Getting started:

```rust
use glass_ui::GlassCard;

let card = GlassCard::new(0.3, 0.4, 0.4, 0.3);
card.blur_radius = 24.0;
card.refraction = 0.60;
renderer.render(&card.to_surfaces());
```

---

Open source on GitHub ⭐
MIT Licensed
Contributions welcome!

github.com/yourusername/liquid-glass

#rustlang #linux #wayland #graphics #opensource
```

### Reddit - r/rust

**Title**: Liquid Glass - Physical optical rendering for Linux (chromatic aberration, Snell refraction, Fresnel effects)

```markdown
Hey r/rust! I've been working on **Liquid Glass**, a glassmorphism rendering engine that simulates real optical physics rather than just blurring and adding transparency.

## What makes it different?

Most glassmorphism (including CSS `backdrop-filter`) just blurs the background. Liquid Glass simulates:

- **Chromatic aberration**: RGB channels separate at glass edges (like a prism)
- **Snell refraction**: Light bends through curved surfaces
- **Fresnel effects**: Edge brightness changes with viewing angle
- **Meniscus lighting**: Liquid-like rim effects

## Tech Stack

- **Rust** for memory safety and performance
- **WebGPU** (wgpu 0.19) for GPU rendering
- **WGSL shaders** for optical calculations
- **Wayland** integration via smithay-client-toolkit
- **Spring physics** for natural animations

## Performance

60+ FPS with multiple surfaces, bounded rendering (no full-screen overdraw).

## Quick Example

```rust
use glass_ui::GlassCard;

let mut card = GlassCard::new(0.3, 0.4, 0.4, 0.3);
card.blur_radius = 24.0;
card.refraction = 0.60;
card.dispersion = 0.020; // Chromatic aberration

renderer.render(&card.to_surfaces());
```

## Current State

- ✅ Core rendering engine
- ✅ Wayland backend
- ✅ Basic UI components (cards, buttons, sliders)
- ✅ Examples (media control, widgets)
- 🚧 X11 backend (planned)
- 🚧 Layout engine (planned)

## Looking for Contributors

Especially interested in help with:
- X11 backend implementation
- More UI components
- Layout engine (flexbox/grid)
- Accessibility features

**GitHub**: github.com/yourusername/liquid-glass
**License**: MIT

[Screenshots showing the examples]

Would love feedback from the Rust community!
```

### Reddit - r/unixporn

**Title**: [Sway] Physical glass rendering with chromatic aberration

```markdown
## Details

- **OS**: Arch Linux
- **WM**: Sway
- **Renderer**: Liquid Glass (custom)
- **Terminal**: (your terminal)
- **Shell**: (your shell)

Built a physically-accurate glass rendering engine for Wayland. Unlike CSS blur, this simulates real optics:

- Chromatic aberration (color separation at edges)
- Snell refraction (light bending)
- Fresnel effects (angle-dependent brightness)
- Meniscus edges (liquid rim lighting)

60+ FPS with WebGPU + WGSL shaders.

Open source (MIT): github.com/yourusername/liquid-glass

## Dotfiles

(if sharing dotfiles)
```

### Hacker News

**Title**: Liquid Glass – Physical glass rendering for Linux (Rust/WebGPU)

```markdown
I've been working on Liquid Glass, a rendering engine that brings physically-accurate glassmorphism to Linux desktops.

Most glassmorphism implementations (including CSS `backdrop-filter`) just blur and add opacity. Liquid Glass simulates actual optical physics:

- Chromatic aberration: RGB channels separate like in a prism
- Snell refraction: Light bends through curved glass surfaces  
- Fresnel effects: Edge brightness varies with viewing angle
- Meniscus lighting: Liquid-like rim glow

Built with Rust and WebGPU for 60+ FPS on Wayland. The WGSL shaders implement physically-based rendering techniques typically seen in 3D engines.

Architecture:
- glass-core: Pure rendering (toolkit-agnostic)
- glass-ui: High-level components
- glass-platform: Wayland/X11 backends

MIT licensed, looking for contributors especially on X11 backend and layout engine.

Screenshots in the repo: github.com/yourusername/liquid-glass
```

### LinkedIn

```markdown
Excited to share my latest open source project: Liquid Glass 🌊

A physically-accurate glassmorphism rendering engine for Linux, bringing macOS-style vibrancy effects to Wayland compositors with real optical simulation.

🔬 Technical highlights:
• Rust for memory safety and performance
• WebGPU for modern GPU pipeline
• WGSL shaders implementing chromatic aberration, Snell refraction, and Fresnel effects
• Wayland layer shell integration
• 60+ FPS with bounded rendering optimization

💡 Why it matters:
Most glassmorphism is approximated with simple blur. Physical simulation creates depth and realism that fake effects can't match.

🚀 Current applications:
• Desktop widgets
• System UI overlays
• Media controls
• Notification panels

Open source (MIT) and welcoming contributors.

#rust #opensource #linux #graphics #softwaredevelopment #webgpu

Link: github.com/yourusername/liquid-glass
```

## Blog Post Draft

### Title: Building a Physical Glass Renderer: Real Optics for Linux UI

**Introduction**

Glassmorphism has become popular in modern UI design, but most implementations fake it. CSS `backdrop-filter` blurs the background and adds opacity—good enough for most cases, but lacking the depth of real glass.

I wanted to see what happened if we simulated actual optical physics. The result is **Liquid Glass**, a rendering engine for Linux that implements chromatic aberration, Snell refraction, and Fresnel effects.

**The Physics**

Real glass isn't just a blur. When light passes through it:

1. **Refraction**: Light bends according to Snell's law (depends on refractive index)
2. **Dispersion**: Different wavelengths bend differently (prisms split white light)
3. **Fresnel**: Reflectivity changes with viewing angle
4. **Edge effects**: Thicker edges trap and guide light (meniscus)

**Implementation**

Written in Rust with WebGPU (wgpu 0.19), the shaders implement these in WGSL:

```wgsl
// Chromatic aberration
let wavelengths = vec3<f32>(650.0, 550.0, 450.0); // RGB in nm
let abbe_number = material.abbe_number;

for each channel:
    let dispersion_offset = calculate_dispersion(wavelength, abbe_number);
    let refracted = refract(view_dir, normal, 1.0 / ior) + dispersion_offset;
    color[channel] = sample_background(uv + refracted.xy);
```

**Performance**

Bounded rendering keeps fragment shader work minimal. Only pixels within the glass surface run the expensive optical calculations.

Results: 60+ FPS with 5-20 surfaces on modest hardware.

**Wayland Integration**

Uses `smithay-client-toolkit` for Wayland and layer shell protocol for overlay rendering. This lets us create floating widgets that capture and blur the desktop behind them in real-time.

**What's Next**

Currently building out:
- More UI components (context menus, modals, input fields)
- X11 backend for wider compatibility
- Layout engine (flexbox/grid)
- Accessibility features

**Try It**

Open source on GitHub: github.com/yourusername/liquid-glass

Would love feedback and contributions!

---

## YouTube Video Script (5 min)

**[0:00-0:30] Hook**
"This is glassmorphism done with real physics. Watch what happens at the edges—that color separation? That's chromatic aberration, the same effect you see in prisms. Most UI just fakes this with blur, but what if we simulated real glass?"

**[0:30-1:30] Demo**
[Screen recording showing examples]
- Media control widget hovering over desktop
- Cursor proximity effects
- Different material presets
- Component showcase cycling

**[1:30-3:00] Technical Explanation**
[Split screen: code + visual result]
- Show WGSL shader code
- Explain chromatic aberration
- Demonstrate refraction bending
- Show Fresnel edge effects

**[3:00-4:00] Performance**
[FPS counter visible]
- Multiple surfaces at 60 FPS
- Bounded rendering explanation
- GPU usage stats

**[4:00-4:45] How to Use**
[Code walkthrough]
```rust
let card = GlassCard::new(...);
card.blur_radius = 24.0;
renderer.render(&card.to_surfaces());
```

**[4:45-5:00] Call to Action**
"Open source on GitHub, link in description. If you build something with it, let me know! Star the repo if you find it useful."

---

## Tags / Keywords

**GitHub Topics**:
rust, wayland, webgpu, graphics, glassmorphism, ui, linux, wgsl, gpu, desktop, widget, rendering, physics, optics

**SEO Keywords**:
- liquid glass linux
- glassmorphism rust
- physical rendering engine
- chromatic aberration ui
- wayland glass effects
- rust webgpu graphics
- linux desktop effects
- transparent blur effects
- real-time refraction
- gpu glass rendering

---

## Outreach

### Communities to Share In

**Reddit**:
- r/rust
- r/unixporn
- r/linux
- r/programming
- r/wayland
- r/archlinux
- r/swaywm

**Forums**:
- Hacker News
- Lobsters
- Dev.to
- Rust Users Forum

**Social**:
- Twitter/X (#rustlang #linux)
- Mastodon (fosstodon.org)
- LinkedIn

**Discord Servers**:
- Rust Programming Language
- Sway/i3
- Linux gaming
- Graphics programming

---

## Press Kit

### One-liner
Physical glass rendering engine for Linux with real optical simulation.

### Short (50 words)
Liquid Glass brings physically-accurate glassmorphism to Linux. Unlike CSS blur, it simulates chromatic aberration, Snell refraction, and Fresnel effects. Built with Rust and WebGPU for 60+ FPS on Wayland. Open source (MIT).

### Medium (150 words)
Liquid Glass is a rendering engine for Linux that implements physically-accurate glass materials using real optical physics. Most glassmorphism effects fake depth with simple blur and opacity. Liquid Glass simulates how light actually interacts with glass: chromatic aberration splits RGB channels like a prism, Snell's law bends light through curved surfaces, Fresnel equations control edge brightness, and meniscus effects create liquid-like rims.

Built with Rust for memory safety and WebGPU for modern GPU pipelines, it achieves 60+ FPS with multiple surfaces through bounded rendering. The Wayland backend uses layer shell protocol for overlay rendering. Applications include desktop widgets, media controls, notification panels, and system UI.

Open source (MIT licensed), the project welcomes contributors especially for X11 backend, layout engine, and accessibility features.

### Contact
- GitHub: github.com/yourusername/liquid-glass
- Email: your.email@example.com
- Twitter: @yourusername
