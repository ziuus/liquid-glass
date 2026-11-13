# Contributing to Liquid Glass

Thank you for your interest in contributing! This document provides guidelines and information for contributors.

## Quick Start

1. **Fork** the repository
2. **Clone** your fork: `git clone https://github.com/ziuus/liquid-glass.git`
3. **Create a branch**: `git checkout -b feature/your-feature-name`
4. **Make changes** and test
5. **Commit**: `git commit -m "feat: add your feature"`
6. **Push**: `git push origin feature/your-feature-name`
7. **Open a Pull Request**

## Areas We Need Help

### 🟢 Good First Issues

#### Documentation
- [ ] Add inline code documentation
- [ ] Write tutorial for creating custom materials
- [ ] Create video walkthrough of examples
- [ ] Translate README to other languages

#### Examples
- [ ] Notification daemon example
- [ ] Context menu widget
- [ ] Weather desktop widget

#### Materials
- [ ] Add more presets (acrylic, mica, etc.)
- [ ] Create material gallery/picker
- [ ] Document optical parameter relationships

### 🟡 Intermediate Issues

#### UI Components
- [ ] GlassDropdown - Dropdown menu
- [ ] GlassModal - Centered dialog overlay
- [ ] GlassContextMenu - Right-click menu

#### Platform Support
- [ ] X11 backend implementation
- [ ] SDL2 window integration improvement
- [ ] Multi-monitor support

#### Performance
- [ ] Surface batching optimization
- [ ] Shader compilation caching
- [ ] Memory profiling

### 🔴 Advanced Issues

#### Core Features
- [ ] Layout engine (flexbox/grid)
- [ ] Event system refactoring
- [ ] Accessibility support (a11y)
- [ ] Animation curves library

#### Advanced Graphics
- [ ] Caustics simulation
- [ ] Internal reflections
- [ ] Volumetric scattering

## Development Setup

### Prerequisites

```bash
# Arch Linux
sudo pacman -S rust wayland wlroots mesa vulkan-tools

# Ubuntu/Debian
sudo apt install rustc cargo wayland-protocols libwayland-dev mesa-vulkan-drivers

# Fedora
sudo dnf install rust cargo wayland-devel mesa-vulkan-drivers
```

### Build

```bash
# Clone
git clone https://github.com/ziuus/liquid-glass.git
cd liquid-glass

# Build all crates
cargo build --workspace --all-targets

# Run tests
cargo test --workspace

# Build the flagship shell
cargo build --release -p liquid-shell

# Build a standalone example
cargo build --release -p glass-panel
```

### Testing Your Changes

```bash
# Run the flagship shell directly
cargo run --release -p liquid-shell

# Run the floating media island
cargo run --release -p glass-panel

# Run the cross-platform window demo
cargo run --release -p simple-window

# Run with debug logging
RUST_LOG=debug cargo run -p liquid-shell

# Run tests with output
cargo test --workspace -- --nocapture

# Check formatting
cargo fmt --all -- --check

# Run clippy (zero warnings required)
cargo clippy --workspace --all-targets -- -D warnings
```

> **Note**: There are no `--example` flags — each demo is a separate `[[bin]]` package under `examples/`. Always use `-p <package-name>` to run them.

## Code Style

### Rust Style Guide

We follow the official Rust style guide with some additions:

```rust
// ✅ Good
pub struct GlassCard {
    pub bounds: [f32; 4],
    pub shape: Shape,
    material: Material,
}

impl GlassCard {
    /// Creates a new glass card at the specified position.
    ///
    /// # Arguments
    /// * `x` - Normalized X coordinate (0.0-1.0)
    /// * `y` - Normalized Y coordinate (0.0-1.0)
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            bounds: [x, y, w, h],
            shape: Shape::RoundedRectangle { radius: 12.0 },
            material: Material::default(),
        }
    }
}

// ❌ Bad
pub struct glasscard {
    pub x:f32,y:f32,w:f32,h:f32,  // No inline multiple fields
    shape:Shape,                   // Missing pub for public fields
}
```

### Shader Style (WGSL)

```wgsl
// ✅ Good
fn calculate_refraction(
    view_dir: vec3<f32>,
    normal: vec3<f32>,
    ior: f32
) -> vec3<f32> {
    return refract(view_dir, normal, 1.0 / ior);
}

// Use descriptive names
let refracted_ray = calculate_refraction(view, n, 1.5);

// ❌ Bad
fn calc(v:vec3<f32>,n:vec3<f32>,i:f32)->vec3<f32>{return refract(v,n,1.0/i);}
```

### Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```bash
# Format
<type>(<scope>): <description>

# Types
feat:     # New feature
fix:      # Bug fix
docs:     # Documentation only
style:    # Code style (formatting, no logic change)
refactor: # Code change that neither fixes a bug nor adds a feature
perf:     # Performance improvement
test:     # Adding or updating tests
chore:    # Maintenance (dependencies, build, etc.)

# Examples
feat(ui): add GlassTextField component
fix(renderer): correct chromatic aberration offset
docs(readme): add installation instructions
perf(shader): optimize blur sampling pattern
```

## Pull Request Process

### Before Submitting

- [ ] Code compiles without warnings (`cargo build --workspace --all-targets`)
- [ ] All tests pass (`cargo test --workspace`)
- [ ] Clippy has zero warnings (`cargo clippy --workspace --all-targets -- -D warnings`)
- [ ] Code is formatted (`cargo fmt --all`)
- [ ] Documentation is updated
- [ ] Examples work if applicable

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Unit tests added/updated
- [ ] Manual testing performed
- [ ] Examples still work

## Screenshots (if applicable)

## Checklist
- [ ] Code follows style guide
- [ ] Self-reviewed the code
- [ ] Commented hard-to-understand areas
- [ ] Documentation updated
- [ ] No new warnings
```

### Review Process

1. **Automated checks** run (CI/CD)
2. **Maintainer review** within 3-5 days
3. **Discussion/changes** if needed
4. **Approval** and merge

## Code of Conduct

### Our Standards

**Positive behavior:**
- Being respectful and inclusive
- Accepting constructive criticism
- Focusing on what's best for the community
- Showing empathy toward others

**Unacceptable behavior:**
- Harassment or discriminatory language
- Trolling, insulting/derogatory comments
- Public or private harassment
- Publishing others' private information
- Other unprofessional conduct

### Enforcement

Violations can be reported via [GitHub Issues](https://github.com/ziuus/liquid-glass/issues). All complaints will be reviewed and investigated.

## Architecture Guidelines

### Crate Organization

```
glass-core/     # Pure rendering engine, no platform code
glass-ui/       # UI components, uses glass-core
glass-platform/ # Platform integration (Wayland, X11)
```

**Dependency rules:**
- `glass-core` depends on: wgpu, bytemuck, serde (no platform libs)
- `glass-ui` depends on: glass-core (no platform libs)
- `glass-platform` depends on: glass-core, wayland/X11 libs

### Adding New UI Components

1. **Define in `glass-ui/src/lib.rs`**:

```rust
pub struct GlassYourComponent {
    pub bounds: [f32; 4],
    // ... fields
}

impl GlassYourComponent {
    pub fn new(...) -> Self { ... }
    pub fn to_surfaces(&self) -> Vec<GlassSurface> { ... }
}
```

2. **Add tests** in the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_your_component() {
        let comp = GlassYourComponent::new(...);
        assert!(comp.bounds[2] > 0.0); // width > 0
    }
}
```

3. **Add to a package** under `examples/` with a working demo.

4. **Document**: Add API docs and usage examples.

### Adding Material Presets

```rust
// In glass-core/src/material/mod.rs

impl GlassMaterial {
    pub fn your_preset() -> Self {
        Self::new(MaterialType::Menu)
            .with_corner_radius(10.0)
            .with_shadow(30.0, [0.0, 4.0], 0.25)
    }
}
```

Document the visual characteristics and use cases.

## Testing

### Unit Tests

```bash
cargo test --workspace
```

### Render Tests (GPU snapshot tests)

```bash
# Full render pipeline integration test
cargo test -p glass-core --test test_render_shader

# UI overlay tests
cargo test -p glass-core --test test_overlay
```

### Visual Benchmarking

The `bench-surfaces` tool is a visual benchmark that renders N glass surfaces and reports FPS. It is **not** a `cargo bench` harness — run it as a binary:

```bash
cargo run --release -p bench-surfaces
# Press space to cycle between 1, 5, 10, 25, 50, 100 surfaces
```

## Resources

### Learning Materials
- [Rust Book](https://doc.rust-lang.org/book/)
- [WebGPU Spec](https://www.w3.org/TR/webgpu/)
- [WGSL Spec](https://www.w3.org/TR/WGSL/)
- [Wayland Protocol](https://wayland.freedesktop.org/docs/html/)

### Similar Projects
- [gtk4-rs](https://gtk-rs.org/) - GTK bindings
- [iced](https://github.com/iced-rs/iced) - Cross-platform GUI
- [egui](https://github.com/emilk/egui) - Immediate mode GUI

### Optical Resources
- [Physically Based Rendering](https://www.pbr-book.org/)
- [Fresnel Equations](https://en.wikipedia.org/wiki/Fresnel_equations)
- [Chromatic Aberration](https://en.wikipedia.org/wiki/Chromatic_aberration)

## Getting Help

- 💬 [GitHub Discussions](https://github.com/ziuus/liquid-glass/discussions)
- 🐛 [Issue Tracker](https://github.com/ziuus/liquid-glass/issues)

## Recognition

Contributors are recognized in:
- GitHub contributors page
- Release notes for significant contributions

---

Thank you for contributing to Liquid Glass! 🌊
