# Contributing to Liquid Glass

First off, thank you for considering contributing to **Liquid Glass**! It's people like you who make open source great.

Whether you are fixing a shader edge case, adding new UI components to `glass-ui`, optimizing Wayland layer shell integration, or writing docs, we welcome your contributions.

---

## 🧭 Project Roadmap & High-Value Contribution Areas

If you're looking for ideas on where to contribute, here are our active focus areas:

1. **`glass-ui` Widgets & Primitives**:
   - Add new components: `GlassTextInput`, `GlassTooltip`, `GlassContextMenu`, `GlassTabBar`.
   - Responsive layouts and nested glass hierarchies.
2. **Platform & Compositor Support (`glass-platform`)**:
   - Hyprland, Sway, KDE Wayland (`org_kde_kwin_blur`), and cosmic-comp background effect protocols.
   - Fractional scaling (`wp_fractional_scale_v1`) support.
   - X11 fallback and macOS Metal/Cocoa backend support.
3. **Optical Shaders (`glass-core`)**:
   - WGSL shader optimizations, compute-based dual-pass Gaussian blur pipeline.
   - Dynamic fluid ripple propagation from mouse drag velocity vectors.
4. **Desktop App Examples**:
   - New showcase apps: Clock & Weather widget, System Resource HUD, Dock bar.

---

## 🛠️ Local Development Setup

### 1. Prerequisites

- **Rust** 1.80+ (`rustup default stable`)
- **Vulkan SDK** / Vulkan driver (`vulkan-radeon`, `vulkan-intel`, or `vulkan-mesa-layers`)
- **Wayland development headers** (`libwayland-dev` on Debian/Ubuntu, `wayland` on Arch)

```bash
# Clone the repository
git clone https://github.com/ziuus/liquid-glass.git
cd liquid-glass

# Verify workspace builds and tests pass
cargo check --workspace
cargo test --workspace
```

---

## 🧪 Testing Your Changes

Before submitting a Pull Request, ensure:

1. **Zero warnings & formatting**:
   ```bash
   cargo fmt --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```
2. **All tests pass**:
   ```bash
   cargo test --workspace
   ```
3. **Run example apps to test UI interactions**:
   ```bash
   # Windowed UI demo (winit + glass-ui)
   cargo run -p simple-window

   # Floating Wayland Media Control Island
   cargo run -p glass-panel --release
   ```

---

## 🔀 Submitting a Pull Request

1. Fork the repo and create your branch from `master`:
   ```bash
   git checkout -b feature/my-cool-feature
   ```
2. Commit your changes with clear, descriptive commit messages.
3. Push to your fork and submit a Pull Request.
4. Describe what your PR adds or fixes and include a screenshot or GIF if it touches visual shaders or UI elements.

---

## 💬 Community & Questions

- Have a feature idea or question? Open a [GitHub Discussion](https://github.com/ziuus/liquid-glass/discussions) or [Issue](https://github.com/ziuus/liquid-glass/issues).
- Found a bug? File an issue with your system info (OS, GPU driver, Wayland compositor).

Thank you for helping make native desktop glass optics gorgeous on Linux! ✨
