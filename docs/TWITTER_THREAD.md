# 𝕏 (Twitter) / Social Media Threads

### Thread Option 1: Architecture & Technical Curiosity (High Engagement)

**Tweet 1:**
Ever wondered why most "glassmorphism" on Linux looks fake?
It's usually just a flat box blur on a rectangle.

I built Liquid Glass: a physical optical lensing engine in Rust + WGPU.
Instead of blurring, it runs Snell's law refraction ($IOR = 1.45$) to bend light rays per-fragment.

**Tweet 2:**
Under the hood:
1. Analytical Signed Distance Fields (SDFs) calculate surface normal gradients ($\nabla h$)
2. Spectral dispersion separates RGB wavelengths at curved meniscus rims
3. Dual-lobe Blinn-Phong specular catches directional sunlight
All in a single GPU quad draw call.

**Tweet 3:**
The coolest part: 0.0% CPU usage when resting.
Using Wayland `epoll_wait` blocking dispatch, the event loop sleeps until pointer events or audio updates occur.

Open source on GitHub:
⭐ https://github.com/ziuus/liquid-glass
Contributions welcome! #rustlang #wayland #graphics

---

### Thread Option 2: Component & Widget Builders

**Tweet 1:**
Building desktop widgets on Wayland without boilerplate:
`liquid-glass` now includes `glass-ui`, a component library with tactile spring physics for buttons, toggles, and scrubber sliders.

Tested at 60 FPS on Hyprland with zero frame drops.
Repo: https://github.com/ziuus/liquid-glass

**Tweet 2:**
First-time contributors welcome!
We just opened several `good first issue` tickets for new UI widgets (`GlassTextInput`, `GlassTooltip`, `GlassContextMenu`):
https://github.com/ziuus/liquid-glass/issues
