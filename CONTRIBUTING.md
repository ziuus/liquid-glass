# Contributing to Liquid Glass

Thank you for your interest in contributing! This document provides guidelines and information for contributors.

## Quick Start

1. **Fork** the repository
2. **Clone** your fork: `git clone https://github.com/yourusername/liquid-glass.git`
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
- [ ] System dock/taskbar example
- [ ] Notification daemon example
- [ ] Context menu widget
- [ ] Desktop widgets (clock, weather, etc.)

#### Materials
- [ ] Add more presets (acrylic, mica, etc.)
- [ ] Create material gallery/picker
- [ ] Document optical parameter relationships

### 🟡 Intermediate Issues

#### UI Components
- [ ] GlassTextField - Text input with glass backing
- [ ] GlassDropdown - Dropdown menu
- [ ] GlassModal - Centered dialog overlay
- [ ] GlassTooltip - Hover tooltip
- [ ] GlassContextMenu - Right-click menu

#### Platform Support
- [ ] X11 backend implementation
- [ ] SDL2 window integration improvement
- [ ] Compositor capability detection
- [ ] Multi-monitor support

#### Performance
- [ ] Surface batching optimization
- [ ] Shader compilation caching
- [ ] Benchmark suite expansion
- [ ] Memory profiling

### 🔴 Advanced Issues

#### Core Features
- [ ] Layout engine (flexbox/grid)
- [ ] Event system refactoring
- [ ] Accessibility support (a11y)
- [ ] Text rendering integration
- [ ] Animation curves library

#### Advanced Graphics
- [ ] Caustics simulation
- [ ] Internal reflections
- [ ] Volumetric scattering
- [ ] Real-time ray tracing mode

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
git clone https://github.com/yourusername/liquid-glass.git
cd liquid-glass

# Build all crates
cargo build --release

# Run tests
cargo test --all

# Run benchmarks
cargo bench

# Build examples
cargo build --release --examples
```

### Testing Your Changes

```bash
# Run a specific example
cargo run --release --example glass-panel

# Run with debug logging
RUST_LOG=debug cargo run --example glass-panel

# Run tests with output
cargo test -- --nocapture

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-targets
```

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

- [ ] Code compiles without warnings
- [ ] All tests pass (`cargo test --all`)
- [ ] Clippy has no warnings (`cargo clippy`)
- [ ] Code is formatted (`cargo fmt`)
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

Violations can be reported to [conduct@example.com]. All complaints will be reviewed and investigated.

## Architecture Guidelines

### Crate Organization

```
glass-core/     # Pure rendering engine, no platform code
glass-ui/       # UI components, uses glass-core
glass-platform/ # Platform integration (Wayland, X11)
```

**Dependency rules:**
- `glass-core` depends on: wgpu, bytemuck, glam (no platform libs)
- `glass-ui` depends on: glass-core (no platform libs)
- `glass-platform` depends on: glass-core, wayland/X11 libs

### Adding New Components

1. **Define in glass-ui/src/lib.rs**:

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

2. **Add tests**:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_your_component() {
        let comp = GlassYourComponent::new(...);
        assert!(comp.is_valid());
    }
}
```

3. **Add example**:

Create `examples/your-component/` with a working demo.

4. **Document**:

Add API docs and usage examples.

### Adding Material Presets

```rust
// In glass-core/src/material.rs

impl Material {
    pub fn your_preset() -> Self {
        Self {
            blur_radius: 20.0,
            refraction: 0.55,
            dispersion: 0.018,
            // ... other properties
        }
    }
}
```

Document the visual characteristics and use cases.

## Testing

### Unit Tests

```bash
cargo test --all
```

### Integration Tests

```bash
cargo test --test integration
```

### Visual Testing

Run examples and visually verify:

```bash
cargo run --release --example glass-panel
# Check for visual artifacts, performance issues
```

### Benchmarks

```bash
cargo bench
# Results in target/criterion/
```

Add benchmarks for performance-critical code:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_render(c: &mut Criterion) {
    c.bench_function("render_surface", |b| {
        let surface = create_test_surface();
        b.iter(|| render(black_box(&surface)));
    });
}

criterion_group!(benches, benchmark_render);
criterion_main!(benches);
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

- 💬 [GitHub Discussions](https://github.com/yourusername/liquid-glass/discussions)
- 🐛 [Issue Tracker](https://github.com/yourusername/liquid-glass/issues)
- 📧 Email: dev@example.com
- 💬 Discord: [Join Server](https://discord.gg/yourserver)

## Recognition

Contributors are recognized in:
- [CONTRIBUTORS.md](CONTRIBUTORS.md) file
- GitHub contributors page
- Release notes for significant contributions

---

Thank you for contributing to Liquid Glass! 🌊
