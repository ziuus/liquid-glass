# AGENTS.md — Liquid Glass

Physical optical lensing and glassmorphism UI engine for Linux & Wayland in Rust (WGPU + WGSL).

## Quick Reference Commands

- **Build all**: `cargo build --workspace --all-targets`
- **Test**: `cargo test --workspace`
- **Lint (zero warnings allowed)**: `cargo clippy --workspace --all-targets -- -D warnings`
- **Format check**: `cargo fmt --all -- --check`
- **Format fix**: `cargo fmt --all`
- **Run floating Wayland demo**: `cargo run --release -p glass-panel`
- **Run cross-platform demo**: `cargo run --release -p simple-window`

## Workspace Structure

```
liquid-glass/
├── glass-core/        # Headless rendering engine: WGPU pipelines, WGSL shaders, RK4 spring physics
│   └── src/           # geometry, material, animation, optical, renderer (glass.wgsl)
├── glass-ui/          # High-level declarative UI widgets (GlassCard, GlassButton, GlassToggle, GlassSlider)
├── glass-platform/    # Wayland/X11 compositor capability detection and adapters
├── examples/
│   ├── glass-panel/   # Wayland layer-shell floating media control island
│   ├── simple-window/ # Cross-platform winit window demo
│   └── sdl-widget/    # SDL2 embedded widget example
├── benchmarks/        # bench-surfaces multi-quad benchmark
└── docs/              # Architectural docs, research, and visual assets
```

## Conventions & Gotchas

1. **`glass-core` is strictly headless**: Never import Wayland, X11, winit, or platform window types in `glass-core`. Platform-specific surface bindings belong in `glass-platform` or `examples/`.
2. **Normalized coordinates**: All `GlassSurface` and `GlassCard` bounds use normalized `[x, y, w, h]` (0.0..1.0) relative to surface dimensions.
3. **Single-pass rendering**: `GlassRenderer::render` expects a slice `&[GlassSurface]`. It renders all active glass elements using tightly bounded quads and dynamic uniform offsets in a single pass.
4. **Shader sync**: `glass.wgsl` uniforms must maintain exact memory alignment with `GlassUniforms` in `glass-core/src/renderer/mod.rs` (std140/WGSL alignment rules).
5. **No full-screen overdraw**: All glass geometry must use clipped bounding quads; do not introduce full-screen blit passes.

## Do Not Modify

- `Cargo.lock` unless dependency changes are explicitly requested.
- `assets/hero.png` and `docs/screenshots/` without explicit user request.
- Release tags or git history.

## Documentation Links

- Architecture: [ARCHITECTURE.md](ARCHITECTURE.md)
- Usage & HIG: [docs/USAGE_GUIDE.md](docs/USAGE_GUIDE.md)
- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)
