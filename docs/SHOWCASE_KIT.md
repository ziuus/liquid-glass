# 🚀 Social Showcase & Community Launch Kit

This kit contains ready-to-publish showcase copy and launch assets for **Reddit, Hacker News, X (Twitter), and Rust Community forums** to drive GitHub stars, users, and contributors to **Liquid Glass**.

---

## 1. 𝕏 (Twitter) / Bluesky Launch Thread

**Post 1 (Hook with media):**
> Built an open-source Liquid Glass & Optical Lensing engine for Linux & Wayland in Rust + WGPU 🦀✨
>
> 💎 Real-time Snell's law refraction ($IOR = 1.33 - 1.52$)  
> 🌈 Spectral chromatic dispersion & meniscus rim lighting  
> ⚡ Underdamped spring kinetics (RK4)  
> 🍃 0.0% CPU when idle via kernel epoll_wait  
>
> Open source & PRs welcome! 👇
>
> [Attach `assets/media_island.gif`]

**Post 2:**
> It ships with:
> • `glass-ui`: High-level interactive components (`GlassCard`, `GlassButton`, `GlassToggle`, `GlassSlider`)
> • `glass-platform`: Wayland layer-shell protocol bindings for floating desktop widgets
> • `glass-core`: Fast analytical WGSL SDF shaders rendered in a single draw call
>
> Repo: https://github.com/ziuus/liquid-glass

**Post 3:**
> Looking for contributors to help expand the widget library (`GlassTextInput`, `GlassTooltip`), compositor protocols (Hyprland, KDE, COSMIC), and new showcase widgets!
>
> Check out the contributing guide: https://github.com/ziuus/liquid-glass/blob/master/CONTRIBUTING.md
>
> Give it a ⭐ if you want to see more high-end native graphics on Linux!

---

## 2. Reddit (`r/rust` & `r/unixporn`)

### For `r/rust`:
**Title:**
> *Liquid Glass: Physical optical lensing & fluid glass UI engine for Linux in Rust & WGPU*

**Body:**
> Hey r/rust!
>
> I've been building **Liquid Glass**, an optical rendering engine and UI toolkit written in Rust, WGPU, and WGSL that brings authentic physical glass refraction and fluid spring physics to Linux desktop apps and Wayland widgets.
>
> ### Why build this?
> Most "frosted glass" implementations on Linux simply apply a flat blur shader to a rectangular texture. Liquid Glass implements true optical physical simulation:
>
> - **Plano-convex Snell Refraction**: Analytical Signed Distance Fields (SDFs) calculate surface normal gradients ($\nabla h$), producing authentic barrel magnification and light bending.
> - **Spectral Chromatic Aberration**: Wavelength splitting ($RGB$ offsets) across curved meniscus edges and chamfered rims.
> - **Zero-cost Idle (0.0% CPU)**: The event loop uses Wayland `epoll_wait` blocking dispatch when at rest, waking only on user interactions or audio updates.
> - **High-Level UI Kit (`glass-ui`)**: Pre-built tactile `GlassButton`, `GlassToggle`, `GlassSlider`, and `GlassCard` driven by sub-stepped Runge-Kutta (RK4) spring physics.
>
> ### Quickstart:
> ```bash
> git clone https://github.com/ziuus/liquid-glass
> cd liquid-glass
> cargo run -p glass-panel --release   # Floating Wayland Media Island
> cargo run -p simple-window          # Windowed desktop UI demo
> ```
>
> We're looking for contributors interested in shaders, UI widget design, and Wayland compositor protocols!
>
> **GitHub:** https://github.com/ziuus/liquid-glass

---

### For `r/unixporn`:
**Title:**
> *[OC] Authentic physical liquid glass floating media island on Wayland (Rust + WGPU, 0% CPU idle)*

**Body:**
> Live demo of a floating Media Control Island running natively on Wayland (`zwlr_layer_shell_v1`).
>
> Instead of a simple box blur, the background is sampled and refracted using analytical Snell's law optics and spectral chromatic dispersion, with an interactive rotating vinyl disc, reactive equalizer bars, and smooth spring physics for dragging anywhere on the desktop.
>
> Everything renders in a single GPU draw call and consumes 0.0% CPU when paused/resting.
>
> Code & documentation: https://github.com/ziuus/liquid-glass

---

## 3. Hacker News (Show HN)

**Title:**
> *Show HN: Liquid Glass – Physical glass optics and UI engine for Linux in Rust*

**Text:**
> Hi HN,
>
> Most glassmorphism on the web and desktop amounts to CSS `backdrop-filter: blur()`. I wanted to see what happens when you implement true physical optical properties—plano-convex Snell's law refraction, chromatic dispersion (prismatic wavelength separation), directional meniscus edge highlights, and fluid harmonic spring kinetics.
>
> The result is **Liquid Glass**, an open-source Rust engine built on WGPU and WGSL:
>
> 1. **Optics Pipeline**: The shader computes analytical SDFs to derive surface normal vectors, refracting the underlying desktop background with barrel distortion and spectral edge splitting.
> 2. **High-Level Component Library (`glass-ui`)**: Primitives for buttons, toggle switches, sliders, and cards with physical spring recoil.
> 3. **Wayland Layer-Shell Integration**: Creates desktop widgets that float seamlessly over wallpapers with real-time sampling and 0.0% CPU usage at idle.
>
> Architecture and code: https://github.com/ziuus/liquid-glass
>
> Would love your thoughts on the shader optics and suggestions for future UI components!
