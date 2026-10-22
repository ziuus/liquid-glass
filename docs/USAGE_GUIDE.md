# Liquid Glass Design System - Usage Guide

## Overview

This guide shows how to use the liquid glass engine to create authentic UI with vibrancy materials, blur effects, spring animations, and proper layering.

## Quick Start

```rust
use glass_core::{
    GlassMaterial, MaterialType, VibrancyMaterial,
    Spring, SpringParams, GlassTransition,
    GlassRenderer, BlurPipeline
};

// Create a sidebar material
let sidebar = GlassMaterial::sidebar();

// Create a menu with custom corner radius
let menu = GlassMaterial::menu()
    .with_corner_radius(8.0)
    .with_shadow(35.0, [0.0, 8.0], 0.3);

// Create a HUD overlay for dark mode
let hud = GlassMaterial::new(MaterialType::HUD)
    .with_dark_mode();
```

## Material System

### Pre-built Materials

The engine includes pre-calibrated vibrancy materials:

```rust
// Window backgrounds - ultra-thin glass
let window = GlassMaterial::new(MaterialType::WindowBackground);
// Blur: 80px, Saturation: 1.5x, Opacity: 0.85

// Sidebars - thick glass with high blur
let sidebar = GlassMaterial::new(MaterialType::Sidebar);
// Blur: 60px, Saturation: 1.6x, Opacity: 0.95

// Menus - frosted glass
let menu = GlassMaterial::new(MaterialType::Menu);
// Blur: 35px, Saturation: 1.55x, Opacity: 0.92

// Popovers - sheet glass
let popover = GlassMaterial::new(MaterialType::Popover);
// Blur: 50px, Saturation: 1.5x, Opacity: 0.90

// Tooltips - minimal glass
let tooltip = GlassMaterial::new(MaterialType::Tooltip);
// Blur: 25px, Saturation: 1.3x, Opacity: 0.95

// HUD overlays - dark thick glass
let hud = GlassMaterial::new(MaterialType::HUD);
// Blur: 70px, Saturation: 1.7x, Dark tint

// Modal sheets
let sheet = GlassMaterial::new(MaterialType::Sheet);
// Blur: 90px, Saturation: 1.6x, Opacity: 0.88
```

### Custom Materials

Create custom vibrancy materials:

```rust
use glass_core::{VibrancyMaterial, BlendMode};

let custom = VibrancyMaterial {
    blur_radius: 45.0,
    saturation_boost: 1.4,
    tint_color: [0.98, 0.98, 0.98, 1.0],
    tint_strength: 0.2,
    opacity: 0.9,
    light_mode: BlendMode::BehindWindow,
    dark_mode: BlendMode::BehindWindow,
    reduced_transparency: false,
};

let glass = GlassMaterial {
    material_type: MaterialType::Custom,
    vibrancy: custom,
    corner_radius: 12.0,
    shadow_blur: 40.0,
    shadow_offset: [0.0, 4.0],
    shadow_opacity: 0.25,
};
```

### Accessibility

Support reduced transparency mode:

```rust
let material = GlassMaterial::menu()
    .with_reduced_transparency();
// Automatically reduces blur to 30%, increases opacity
```

## Animation System

### Spring Physics

All animations use spring physics for natural motion:

```rust
use glass_core::{Spring, SpringParams};

// Default UI spring (smooth, quick settling)
let mut opacity = Spring::new(1.0);
opacity.params = SpringParams::default_ui();

// Set target and update each frame
opacity.set_target(0.5);

// In your render loop
let delta_time = 1.0 / 60.0; // 60 FPS
opacity.update(delta_time);

// Use current value
let current_opacity = opacity.value;
```

### Spring Presets

```rust
// Bouncy - playful interactions
SpringParams::bouncy()
// Stiffness: 200, Damping: 0.6

// Smooth - gentle transitions
SpringParams::smooth()
// Stiffness: 300, Damping: 0.85

// Snappy - immediate response
SpringParams::snappy()
// Stiffness: 400, Damping: 0.8

// Gentle - large motions
SpringParams::gentle()
// Stiffness: 180, Damping: 0.9
```

### Multi-dimensional Springs

```rust
use glass_core::{SpringVec2, SpringVec4};

// Position animation
let mut position = SpringVec2::new([0.0, 0.0]);
position.set_target([100.0, 50.0]);

// Color animation
let mut color = SpringVec4::with_params(
    [1.0, 1.0, 1.0, 1.0],
    SpringParams::smooth()
);
color.set_target([0.5, 0.7, 1.0, 0.9]);

// Update both
position.update(delta_time);
color.update(delta_time);
```

### Coordinated Glass Transitions

Animate blur, opacity, and scale together:

```rust
use glass_core::GlassTransition;

let mut transition = GlassTransition::new();

// Hover in
transition.hover_in(60.0); // base blur radius
// Sharpens blur to 54px, opacity to 0.95, scale to 1.02

// Hover out
transition.hover_out(60.0);
// Returns to original values

// Press
transition.press(60.0);
// More blur (66px), less opacity (0.85), scale down (0.97)

// Release
transition.release(60.0);
// Returns to original

// Update
transition.update(delta_time);

// Use values
let blur = transition.blur_radius.value;
let opacity = transition.opacity.value;
let scale = transition.scale.value;
```

## Blur Pipeline

### Multi-pass Gaussian Blur

The engine uses a multi-pass pipeline for physical-quality blur:

```rust
use glass_core::BlurPipeline;

// Create blur pipeline
let blur_pipeline = BlurPipeline::new(&device, width, height);

// Apply blur in render pass
let blurred_view = blur_pipeline.apply_blur(
    &mut encoder,
    &source_texture_view,
    60.0,  // blur radius
    1.5,   // saturation boost
);
```

The pipeline automatically:
1. Downsamples to half resolution
2. Applies separable Gaussian blur (horizontal + vertical)
3. Boosts saturation for vibrancy
4. Upsamples for final output

### Blur Quality Settings

```rust
// Light blur - tooltips, minimal UI
blur_radius: 25.0,
saturation_boost: 1.3,

// Medium blur - menus, popovers
blur_radius: 35.0-50.0,
saturation_boost: 1.5,

// Heavy blur - windows, sheets
blur_radius: 80.0-100.0,
saturation_boost: 1.6-1.8,
```

## Complete Examples

### Button with Glass Material

```rust
struct GlassButton {
    material: GlassMaterial,
    transition: GlassTransition,
    position: [f32; 2],
    size: [f32; 2],
    is_hovered: bool,
    is_pressed: bool,
}

impl GlassButton {
    fn new() -> Self {
        Self {
            material: GlassMaterial::menu()
                .with_corner_radius(6.0)
                .with_shadow(20.0, [0.0, 2.0], 0.2),
            transition: GlassTransition::new(),
            position: [0.0, 0.0],
            size: [120.0, 40.0],
            is_hovered: false,
            is_pressed: false,
        }
    }
    
    fn on_hover_enter(&mut self) {
        self.is_hovered = true;
        self.transition.hover_in(self.material.vibrancy.blur_radius);
    }
    
    fn on_hover_exit(&mut self) {
        self.is_hovered = false;
        self.transition.hover_out(self.material.vibrancy.blur_radius);
    }
    
    fn on_press(&mut self) {
        self.is_pressed = true;
        self.transition.press(self.material.vibrancy.blur_radius);
    }
    
    fn on_release(&mut self) {
        self.is_pressed = false;
        if self.is_hovered {
            self.transition.hover_in(self.material.vibrancy.blur_radius);
        } else {
            self.transition.release(self.material.vibrancy.blur_radius);
        }
    }
    
    fn update(&mut self, delta_time: f32) {
        self.transition.update(delta_time);
    }
    
    fn render(&self, renderer: &mut GlassRenderer) {
        // Apply animated values
        let mut material = self.material;
        material.vibrancy.blur_radius = self.transition.blur_radius.value;
        material.vibrancy.opacity = self.transition.opacity.value;
        
        renderer.draw_glass_rect(
            self.position,
            self.size,
            &material,
            self.transition.scale.value,
        );
    }
}
```

### Sidebar with Layered Glass

```rust
struct GlassSidebar {
    background: GlassMaterial,
    items: Vec<SidebarItem>,
    position_spring: SpringVec2,
    width: f32,
}

impl GlassSidebar {
    fn new() -> Self {
        Self {
            background: GlassMaterial::sidebar()
                .with_corner_radius(10.0)
                .with_shadow(30.0, [0.0, 2.0], 0.2),
            items: Vec::new(),
            position_spring: SpringVec2::with_params(
                [0.0, 0.0],
                SpringParams::smooth()
            ),
            width: 250.0,
        }
    }
    
    fn slide_in(&mut self) {
        self.position_spring.set_target([0.0, 0.0]);
    }
    
    fn slide_out(&mut self) {
        self.position_spring.set_target([-self.width, 0.0]);
    }
    
    fn update(&mut self, delta_time: f32) {
        self.position_spring.update(delta_time);
    }
    
    fn render(&self, renderer: &mut GlassRenderer) {
        let pos = self.position_spring.value();
        
        // Render shadow first
        renderer.draw_glass_shadow(
            pos,
            [self.width, 800.0],
            &self.background,
        );
        
        // Render glass background
        renderer.draw_glass_rect(
            pos,
            [self.width, 800.0],
            &self.background,
            1.0,
        );
        
        // Render items on top
        for item in &self.items {
            item.render(renderer);
        }
    }
}
```

### Modal with Sheet Material

```rust
struct GlassModal {
    sheet: GlassMaterial,
    backdrop_opacity: Spring,
    scale: Spring,
    is_visible: bool,
}

impl GlassModal {
    fn new() -> Self {
        Self {
            sheet: GlassMaterial::new(MaterialType::Sheet)
                .with_corner_radius(12.0)
                .with_shadow(60.0, [0.0, 20.0], 0.4),
            backdrop_opacity: Spring::with_params(0.0, SpringParams::smooth()),
            scale: Spring::with_params(0.9, SpringParams::snappy()),
            is_visible: false,
        }
    }
    
    fn show(&mut self) {
        self.is_visible = true;
        self.backdrop_opacity.set_target(0.5);
        self.scale.set_target(1.0);
    }
    
    fn hide(&mut self) {
        self.is_visible = false;
        self.backdrop_opacity.set_target(0.0);
        self.scale.set_target(0.9);
    }
    
    fn update(&mut self, delta_time: f32) {
        self.backdrop_opacity.update(delta_time);
        self.scale.update(delta_time);
    }
    
    fn render(&self, renderer: &mut GlassRenderer) {
        if !self.is_visible && self.backdrop_opacity.is_at_rest() {
            return;
        }
        
        // Dark backdrop
        renderer.draw_rect(
            [0.0, 0.0],
            renderer.screen_size(),
            [0.0, 0.0, 0.0, self.backdrop_opacity.value],
        );
        
        // Centered modal
        let screen_size = renderer.screen_size();
        let modal_size = [600.0, 400.0];
        let center = [
            (screen_size[0] - modal_size[0]) * 0.5,
            (screen_size[1] - modal_size[1]) * 0.5,
        ];
        
        renderer.draw_glass_rect(
            center,
            modal_size,
            &self.sheet,
            self.scale.value,
        );
    }
}
```

## Dark Mode Support

```rust
// Detect system appearance
let is_dark_mode = detect_system_appearance();

// Apply to materials
let material = if is_dark_mode {
    GlassMaterial::sidebar().with_dark_mode()
} else {
    GlassMaterial::sidebar()
};
```

## Performance Tips

1. **Cache blur results**: Blur is expensive - cache when background doesn't change
2. **Use lower resolution**: Blur at half or quarter resolution
3. **Limit layers**: Each glass layer requires a blur pass
4. **Batch similar materials**: Group by blur radius to reduce passes
5. **Skip animations at rest**: Check `spring.is_at_rest()` before updating

## Migration from Old API

```rust
// OLD (deprecated)
let material = Material::LiquidGlass;
let params = MaterialParams {
    ior: 1.33,
    dispersion: 0.08,
    // ... complex physics params
};

// NEW (Physical)
let material = GlassMaterial::new(MaterialType::WindowBackground);
// Pre-calibrated for authentic look
```

## Best Practices

1. **Use presets first**: The built-in materials match physical's design language
2. **Animate with springs**: Never use linear interpolation for glass transitions
3. **Layer properly**: Background blur → Glass → Foreground content
4. **Mind the shadows**: Shadows should be large and soft (40-60px blur)
5. **Test in both modes**: Always check light and dark appearance
6. **Support accessibility**: Use `with_reduced_transparency()` when needed
7. **Keep blur radii reasonable**: 25-100px range for most UI

## Common Patterns

### Hover Effect
```rust
transition.hover_in(base_blur);  // Sharpen + scale up
```

### Press Effect
```rust
transition.press(base_blur);  // More blur + scale down
```

### Slide-in Panel
```rust
position_spring.set_target([0.0, 0.0]);  // Smooth entrance
```

### Fade Modal
```rust
opacity.set_target(1.0);  // Spring-based fade
```

## Troubleshooting

**Blur looks pixelated**
- Increase blur texture resolution
- Check saturation boost is applied
- Ensure using Gaussian blur, not box blur

**Performance issues**
- Reduce blur radius
- Use lower resolution blur textures
- Cache blur results when possible
- Limit number of glass layers

**Colors look washed out**
- Increase saturation_boost (try 1.5-1.8)
- Check tint_strength isn't too high
- Verify blur pipeline includes saturation pass

**Animations feel stiff**
- Adjust damping (lower = more bounce)
- Increase stiffness for faster response
- Check delta_time calculation

## Next Steps

- See `examples/glass-panel/` for complete working examples
- Check `LIQUID_GLASS_ANALYSIS.md` for technical details
- Read the API docs for advanced usage
