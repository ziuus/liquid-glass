# Apple Liquid Glass Design System Analysis

## Core Principles (Based on Apple's Human Interface Guidelines & Observable Behavior)

### 1. Material Hierarchy
Apple's liquid glass isn't just about blur - it's a layered material system:

- **Base Materials**:
  - Ultra-thin glass (windows, sheets)
  - Thick glass (sidebars, toolbars) 
  - Frosted glass (menus, popovers)
  - Vibrancy materials (NSVisualEffectView materials)

- **Material Properties**:
  - Thickness affects blur radius and light transmission
  - Materials respond to content behind them (vibrancy)
  - Edge treatments vary by hierarchy (sharp vs soft)

### 2. Blur System
Apple uses multi-pass blur with specific characteristics:

- **Gaussian blur** as base (not simple box blur)
- **Saturation boost** on blurred content (makes colors pop through glass)
- **Luminance-aware** blur (preserves light/dark contrast)
- **Adaptive blur radius**: 
  - Menus: ~30-40px
  - Sidebars: ~60-80px  
  - Full window materials: ~80-100px

### 3. Vibrancy & Light Adaptation
Key differentiator from generic blur:

- **Content-aware tinting**: Material color shifts based on background
- **Light/Dark mode adaptation**: Materials become more opaque in dark mode
- **Behind-window blending**: True compositing with desktop content
- **Reduced transparency mode**: Honors accessibility settings

### 4. Layering & Depth
Apple creates depth through:

- **Multiple glass layers** with different blur radii
- **Shadow system**: Soft, large-radius shadows between layers
- **Elevation levels**: Each level has specific material + shadow
- **Parallax on hover** (subtle, 1-2px max)

### 5. Edge Treatment
Critical for "liquid" feel:

- **Anti-aliased rounded corners** (variable radius by component)
- **Stroke/border handling**:
  - Inner glow for elevated surfaces
  - Outer glow for floating elements
  - No harsh pixel borders
- **Corner radius by type**:
  - Windows: 10-12px
  - Popovers: 8px
  - Buttons: 4-6px
  - Inline elements: 2-4px

### 6. Lighting Model
Apple's glass interacts with light:

- **Specular highlights** on glass edges
- **Fresnel effect** (edge brightness)
- **Rim lighting** from "environment"
- **Directional light** (implicit top-down light source)

### 7. Animation Characteristics
Liquid glass movement:

- **Spring physics** (not linear easing)
  - Damping: 0.7-0.85
  - Stiffness: 200-300
  - Mass: 0.8-1.2
- **Blur radius animates** during transitions
- **Material opacity fades** with motion
- **Coordinate with shadows** (shadows fade faster than blur)

### 8. Interaction Feedback
Touch/hover responses:

- **Scale on press**: 0.95-0.98x (subtle)
- **Opacity pulse**: 0.8-0.9 on active
- **Blur reduction**: Sharpen slightly on hover
- **Color shift**: Slight saturation boost on interaction

## Current Engine Gaps

### What's Missing:
1. **True vibrancy system** - currently just basic blur
2. **Multi-pass blur** - single-pass doesn't match Apple quality
3. **Saturation boost** on blurred content
4. **Content-aware material adaptation**
5. **Proper shadow system** coordinated with materials
6. **Spring physics** for animations
7. **Accessibility modes** (reduced transparency)
8. **Efficient compositing** for real-time background capture

### What's Present but Needs Refinement:
1. Physical refraction model (too complex for UI)
2. Chromatic dispersion (not used in Apple's UI glass)
3. Edge lighting (good but needs tuning)
4. Basic material types (need proper parameter sets)

## Recommended Architecture Changes

### 1. Material System Redesign
```rust
pub enum MaterialType {
    // Apple's actual materials
    WindowBackground,      // Ultra-thin glass
    Sidebar,              // Thick glass, high blur
    Titlebar,            // Medium glass
    Menu,                // Frosted glass
    Popover,             // Sheet glass
    Tooltip,             // Minimal glass
    HUD,                 // Dark thick glass
    Sheet,               // Modal glass
    UnderPageBackground, // Content-behind
}

pub struct VibrancyMaterial {
    blur_radius: f32,           // Gaussian blur radius
    saturation_boost: f32,      // 1.0-1.8 range
    tint_color: [f32; 4],      // Base material tint
    tint_strength: f32,         // How much tint vs background
    opacity: f32,               // Overall opacity
    light_mode: BlendMode,      // How to blend in light mode
    dark_mode: BlendMode,       // How to blend in dark mode
}
```

### 2. Blur Pipeline
Multi-pass system:
1. Capture background (screenshot or compositor integration)
2. Downsample (1/2 or 1/4 resolution)
3. Gaussian blur (separable, 2-pass)
4. Saturation boost
5. Upsample with material tint
6. Composite with foreground

### 3. Shader Optimizations
- Remove complex refraction for UI elements
- Simplified lighting model (directional + rim)
- Efficient blur (dual kawase or dual filter)
- Proper alpha compositing

### 4. Component Library
Pre-built material configurations:
- Window materials
- Button materials  
- Panel materials
- Overlay materials
- Each with proper blur/tint/shadow

### 5. Animation System
- Spring-based interpolation
- Coordinated blur/opacity/scale
- Gesture-driven physics
- Proper easing curves

## Implementation Priority

### Phase 1: Core Material System
1. Implement multi-pass Gaussian blur
2. Add saturation boost stage
3. Create material presets
4. Background capture system

### Phase 2: Visual Refinement
1. Proper shadow rendering
2. Edge treatment (rounded corners, glows)
3. Lighting model simplification
4. Anti-aliasing improvements

### Phase 3: Motion & Interaction
1. Spring physics animation
2. Hover/press states
3. Transition choreography
4. Gesture response

### Phase 4: Optimization
1. Blur caching
2. Mipmap-based blur
3. Adaptive quality
4. Compositor integration

### Phase 5: Accessibility
1. Reduced transparency mode
2. High contrast support
3. Color adjustments
4. Performance fallbacks

## Key Metrics for "Liquid Glass" Feel

- Blur must feel **soft and diffused**, not pixelated
- Colors should **glow through** the glass (saturation)
- Edges must be **perfectly anti-aliased**
- Shadows should be **large and subtle**
- Motion should feel **springy and alive**
- Materials should **adapt to background**
- Performance: 60fps minimum, 120fps target
