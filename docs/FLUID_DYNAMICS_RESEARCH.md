# Liquid Glass - True Fluid Dynamics Research

## What Real Liquid Glass Actually Is

**Liquid Glass** = Glass components that behave like **liquid droplets**:

### Core Behaviors:

1. **Surface Tension**
   - Edges naturally curve inward (meniscus)
   - Forms smooth, organic boundaries
   - Self-sealing perimeter

2. **Fluid Deformation**
   - Stretches when moving (like pulling taffy)
   - Bounces back with spring physics
   - Ripples propagate from interaction points

3. **Gravitational Response**
   - Bottom edge slightly heavier (droplet sag)
   - Top edge lighter, more curved
   - Asymmetric curvature

4. **Motion Dynamics**
   - **Trailing effect**: When moving, leaves "tail" like water
   - **Leading edge**: Compresses/bulges ahead of motion
   - **Ripple waves**: Oscillations from momentum changes
   - **Settling**: Wobbles before coming to rest

5. **Interactive Response**
   - Click = ripple emanates from point
   - Hover = localized surface distortion
   - Drag = stretches the entire droplet
   - Release = elastic snap-back with decay

6. **Edge Characteristics**
   - Thicker at contact edge (capillary effect)
   - Refractive distortion at boundaries
   - Light caustics from curved surface
   - Specular highlights on "wet" surface

## Implementation Approach

### Shader Side:
1. **Dynamic SDF** - Signed distance field that morphs
2. **Vertex displacement** - Animate mesh positions
3. **Time-based oscillation** - Sine waves for wobble
4. **Velocity-driven deformation** - Stretch based on movement
5. **Edge thickness variation** - Thicker at bottom (gravity)

### CPU Side:
1. **Physics simulation** - Track velocity, acceleration
2. **Spring damping** - Oscillation with decay
3. **Ripple propagation** - Wave equations
4. **Momentum tracking** - For trailing effects

### Key Parameters:
- **Surface tension**: 0.7 (higher = rounder droplet)
- **Viscosity**: 0.3 (controls flow resistance)
- **Elasticity**: 0.8 (bounce-back strength)
- **Damping**: 0.85 (oscillation decay)
- **Gravity**: 0.15 (subtle downward sag)

## Examples in macOS:

**Menu Dropdowns**:
- Animate in with elastic bounce
- Edges wobble slightly on arrival
- Settle with damped oscillation

**Notification Banners**:
- Slide in with fluid trailing edge
- Compress slightly on stop
- Ripple on interaction

**Sheets**:
- Scale up with surface tension
- Edges stay curved throughout
- Dismiss with fluid shrink

## The "Alive" Feeling:

The droplet feels **alive** because:
1. Never perfectly still (subtle breathing)
2. Responds immediately to input
3. Over-shoots then settles (natural physics)
4. Maintains cohesion (doesn't break apart)
5. Reacts contextually (hover vs click vs drag)

This is NOT just animation curves - it's real-time physics simulation of fluid behavior!
