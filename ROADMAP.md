# Liquid Glass Roadmap

## v0.1.x (Current)
- Physical Snell refraction, Cauchy chromatic dispersion, and RK4 spring physics on WGPU + WGSL
- `GlassRenderer::set_power_save` (1-tap shader Eco mode)
- `UIFont::global_scale` customizable typography scaling
- `crates.io` packaging for `glass-core`, `glass-platform`, and `glass-ui`

## v0.2 (Next)
- `GlassTextInput` & `GlassContextMenu` widgets in `glass-ui`
- KDE KWin & COSMIC layer-shell / blur protocol helpers in `glass-platform`
- Automated `simple-demo` binary releases via GitHub Actions

## v0.3
- Plugin trait for third-party widgets
- Windows DX12 live-backdrop capture support
- Stable `1.0` API freeze & comprehensive `docs.rs` examples
