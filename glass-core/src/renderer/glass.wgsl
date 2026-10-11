// Liquid Glass WGSL Shader
// True Physical Liquid Droplet & Convex Lensing Engine:
//   - Convex squircle dome + meniscus height-field: continuous non-flat normal gradient
//   - Plano-convex optical magnification: genuine optical zoom under droplets
//   - Directional illumination only: no fake 360° white outlines
//   - Caustic rim refraction & bottom ambient occlusion
//   - Dual-source Blinn-Phong specular glints
//   - Dynamic fluid ripple advection with surface tension
//   - Procedural etched glass iconography (equalizer, gauges, play/pause, scrubbers)
//   - Contact drop shadows for multi-layer depth

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct GlassUniforms {
    bounds: vec4<f32>,
    tint: vec4<f32>,
    
    ior: f32,
    roughness: f32,
    refraction_strength: f32,
    dispersion: f32,
    
    fresnel_strength: f32,
    specular_strength: f32,
    blur_radius: f32,
    curvature: f32,
    
    bevel: f32,
    time: f32,
    deformation: f32,
    corner_radius: f32,
    
    pointer_pos: vec2<f32>,
    pointer_velocity: vec2<f32>,
    
    surface_size: vec2<f32>,
    padding2: vec2<f32>, // x = element_type (0=base, 1=header, 2=wave, 3=gauge, 4=disc, 5=prev, 6=play, 7=next, 8=slider), y = progress
};

@group(0) @binding(0) var<uniform> params: GlassUniforms;
@group(1) @binding(0) var background_texture: texture_2d<f32>;
@group(1) @binding(1) var background_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    var local_uv = vec2<f32>(0.0);
    switch in_vertex_index {
        case 0u: { local_uv = vec2<f32>(0.0, 0.0); }
        case 1u: { local_uv = vec2<f32>(0.0, 1.0); }
        case 2u: { local_uv = vec2<f32>(1.0, 0.0); }
        case 3u: { local_uv = vec2<f32>(1.0, 0.0); }
        case 4u: { local_uv = vec2<f32>(0.0, 1.0); }
        case 5u: { local_uv = vec2<f32>(1.0, 1.0); }
        default: {}
    }
    
    out.uv = local_uv;
    out.clip_position = vec4<f32>(local_uv.x * 2.0 - 1.0, 1.0 - local_uv.y * 2.0, 0.0, 1.0);
    return out;
}

// ── Rounded-rect Signed Distance Field ──
fn rr_sdf(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

// ── Continuous Liquid Dome + Meniscus Height Profile ──
// Combines steep edge meniscus with a continuous convex lens dome across the entire droplet
fn droplet_height(p: vec2<f32>, b: vec2<f32>, r: f32, z_radius: f32) -> f32 {
    let d = -rr_sdf(p, b, r);
    if d <= 0.0 {
        return 0.0;
    }
    
    // 1. Edge meniscus curve (steep quadratic rim)
    let meniscus_w = max(z_radius, 4.0);
    let rim_t = clamp(d / meniscus_w, 0.0, 1.0);
    let h_rim = sqrt(rim_t * (2.0 - rim_t)) * meniscus_w;
    
    // 2. Central convex lens dome (smooth squircle bulge)
    let norm_p = clamp(p / max(b, vec2<f32>(1.0)), vec2<f32>(-1.0), vec2<f32>(1.0));
    let rad_sq = dot(norm_p, norm_p);
    let h_dome = sqrt(max(0.0, 1.0 - rad_sq * 0.85)) * (z_radius * 0.5);
    
    return h_rim * 0.65 + h_dome * 0.35;
}

// ── 48-tap Vogel Golden Spiral Creamy Blur ──
fn blurred_sample(uv: vec2<f32>, radius: f32) -> vec3<f32> {
    if radius <= 0.5 {
        return textureSampleLevel(background_texture, background_sampler, uv, 0.0).rgb;
    }
    
    var color = vec3<f32>(0.0);
    let samples = 48.0;
    let golden_angle = 2.39996323;
    
    for (var i = 0.0; i < samples; i += 1.0) {
        let r = sqrt(i + 0.5) / sqrt(samples);
        let theta = i * golden_angle;
        let offset = vec2<f32>(r * cos(theta), r * sin(theta)) * radius;
        let sample_uv = clamp(uv + offset / params.surface_size, vec2<f32>(0.001), vec2<f32>(0.999));
        color += textureSampleLevel(background_texture, background_sampler, sample_uv, 0.0).rgb;
    }
    
    return color / samples;
}

// ── Fluid pointer ripple & advection stretch ──
fn liquid_displacement(world_px: vec2<f32>) -> vec2<f32> {
    var dp = vec2<f32>(0.0);
    
    // Continuous ambient breathing wave
    if params.deformation > 0.0 {
        dp.x += sin(world_px.y * 0.015 + params.time * 1.6) * params.deformation * 5.0;
        dp.y += cos(world_px.x * 0.015 + params.time * 1.3) * params.deformation * 5.0;
    }
    
    // Interactive cursor wave & standing meniscus tension
    let pointer_px = params.pointer_pos * params.surface_size;
    let dist = distance(world_px, pointer_px);
    let effect_radius = 160.0;
    
    if dist < effect_radius && params.pointer_pos.x >= 0.0 {
        let t = 1.0 - (dist / effect_radius);
        let t2 = t * t;
        
        // Moving ripple advection
        let vel = params.pointer_velocity * 24.0;
        let vel_mag = clamp(length(vel), 0.0, 1.0);
        if vel_mag > 0.0001 {
            let ripple = sin(dist * 0.07 - params.time * 14.0) * exp(-dist * 0.03);
            let advection = vel * t2 * 12.0;
            let dir = normalize(world_px - pointer_px + vec2<f32>(0.001));
            dp += advection + dir * ripple * 8.0 * vel_mag * t;
        }
        
        // Viscous surface tension pull towards cursor (gentle organic attraction)
        let pull_dir = normalize(pointer_px - world_px + vec2<f32>(0.001));
        dp += pull_dir * sin(t * 3.14159) * 2.2;
    }
    
    return dp;
}

// ── Struct for Procedural Liquid Element Core & Glyph ──
struct ElementAppearance {
    glyph_mask: f32,
    fluid_color: vec3<f32>,
    fluid_glow: f32,
};

// ── Segment distance helper for crisp vector icons ──
fn dist_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// ── Authentic Apple macOS Liquid Glass Controls & Glyphs ──
fn render_element_appearance(local_p: vec2<f32>, half: vec2<f32>, el_type: f32, progress: f32, time: f32) -> ElementAppearance {
    var app: ElementAppearance;
    app.glyph_mask = 0.0;
    app.fluid_color = vec3<f32>(1.0, 1.0, 1.0);
    app.fluid_glow = 0.0;
    
    if el_type <= 0.0 {
        return app;
    }
    
    // 1. Dynamic Island Header Pill: Live Emerald Status Beacon + Status Pill
    if el_type >= 0.9 && el_type <= 1.1 {
        // Emerald live beacon with gentle breathing pulse
        let beacon_pos = local_p - vec2<f32>(-half.x + 18.0, 0.0);
        let pulse = sin(time * 3.5) * 0.4 + 1.0;
        let d_beacon = length(beacon_pos) - 3.2;
        let beacon = 1.0 - smoothstep(-0.6, 0.6, d_beacon);
        
        // Mini cellular signal bars on the right
        let b1 = rr_sdf(local_p - vec2<f32>(half.x - 30.0, 3.0), vec2<f32>(1.5, 3.0), 0.8);
        let b2 = rr_sdf(local_p - vec2<f32>(half.x - 22.0, 1.0), vec2<f32>(1.5, 5.0), 0.8);
        let b3 = rr_sdf(local_p - vec2<f32>(half.x - 14.0, -1.0), vec2<f32>(1.5, 7.0), 0.8);
        let bars = 1.0 - smoothstep(-0.5, 0.5, min(b1, min(b2, b3)));
        
        app.glyph_mask = max(beacon, bars * 0.85);
        app.fluid_color = mix(vec3<f32>(0.2, 0.92, 0.55), vec3<f32>(1.0), bars);
        app.fluid_glow = beacon * 0.9 * pulse;
        return app;
    }
    
    // 2. Wi-Fi Glass Capsule: 3 Concentric Apple Wave Arcs + Base Dot + Radiant Cyan Fluid
    if el_type >= 1.9 && el_type <= 2.1 {
        let center_offset = vec2<f32>(0.0, 9.0);
        let p_rel = local_p - center_offset;
        let r = length(p_rel);
        
        // Base dot
        let dot_dist = r - 2.8;
        let dot_mask = 1.0 - smoothstep(-0.6, 0.6, dot_dist);
        
        // 3 concentric arcs with 90° angular fan cutoff
        let arc1 = abs(r - 10.0) - 1.6;
        let arc2 = abs(r - 18.0) - 1.6;
        let arc3 = abs(r - 26.0) - 1.6;
        let min_arc = min(arc1, min(arc2, arc3));
        
        let angle = atan2(p_rel.y, abs(p_rel.x));
        let in_wedge = select(0.0, 1.0, angle < -0.85);
        let arcs_mask = (1.0 - smoothstep(-0.6, 0.6, min_arc)) * in_wedge;
        
        app.glyph_mask = max(dot_mask, arcs_mask);
        app.fluid_color = vec3<f32>(0.05, 0.72, 1.0); // Vibrant Apple Cyan
        app.fluid_glow = 0.45; // Internal bioluminescent fluid core
        return app;
    }
    
    // 3. Bluetooth Glass Capsule: Apple Bluetooth Rune + Radiant Indigo Fluid
    if el_type >= 2.9 && el_type <= 3.1 {
        // Vertical spine
        let d_spine = dist_segment(local_p, vec2<f32>(0.0, -16.0), vec2<f32>(0.0, 16.0)) - 1.5;
        // Upper chevron lobe
        let d_up1 = dist_segment(local_p, vec2<f32>(0.0, -16.0), vec2<f32>(9.0, -8.0)) - 1.5;
        let d_up2 = dist_segment(local_p, vec2<f32>(9.0, -8.0), vec2<f32>(-8.0, 8.0)) - 1.5;
        // Lower chevron lobe
        let d_dn1 = dist_segment(local_p, vec2<f32>(0.0, 16.0), vec2<f32>(9.0, 8.0)) - 1.5;
        let d_dn2 = dist_segment(local_p, vec2<f32>(9.0, 8.0), vec2<f32>(-8.0, -8.0)) - 1.5;
        
        let rune = min(d_spine, min(min(d_up1, d_up2), min(d_dn1, d_dn2)));
        app.glyph_mask = 1.0 - smoothstep(-0.6, 0.6, rune);
        app.fluid_color = vec3<f32>(0.25, 0.45, 1.0); // Electric Indigo/Sapphire
        app.fluid_glow = 0.45;
        return app;
    }
    
    // 4. Display Brightness Liquid Slider: Sun Icon + Golden/Amber Sloshing Fluid Column
    if el_type >= 3.9 && el_type <= 4.1 {
        // Sun icon on the left
        let sun_center = vec2<f32>(-half.x + 24.0, 0.0);
        let sun_p = local_p - sun_center;
        let sun_r = length(sun_p);
        let sun_core = 1.0 - smoothstep(-0.6, 0.6, sun_r - 5.5);
        
        // 8 radiating sun rays
        var rays = 0.0;
        for (var i = 0.0; i < 8.0; i += 1.0) {
            let theta = i * 0.785398; // pi / 4
            let ray_start = vec2<f32>(cos(theta), sin(theta)) * 8.5;
            let ray_end = vec2<f32>(cos(theta), sin(theta)) * 12.0;
            let d_ray = dist_segment(sun_p, ray_start, ray_end) - 1.2;
            rays = max(rays, 1.0 - smoothstep(-0.5, 0.5, d_ray));
        }
        let sun_icon = max(sun_core, rays);
        
        // Dynamic sloshing liquid fluid column
        let track_start = -half.x + 4.0;
        let track_width = half.x * 2.0 - 8.0;
        let fill_x = track_start + track_width * clamp(progress, 0.05, 0.98);
        
        // Liquid wave meniscus profile
        let slosh = sin(local_p.y * 0.18 + time * 3.5) * 2.2;
        let meniscus_x = fill_x + slosh;
        
        // Translucent liquid fluid filling
        let in_liquid = select(0.0, 1.0, local_p.x <= meniscus_x);
        let meniscus_rim = 1.0 - smoothstep(0.0, 2.5, abs(local_p.x - meniscus_x));
        
        app.glyph_mask = max(sun_icon * 0.95, meniscus_rim * 0.8);
        app.fluid_color = vec3<f32>(1.0, 0.76, 0.22); // Radiant Warm Amber/Gold
        app.fluid_glow = in_liquid * 0.38;
        return app;
    }
    
    // 5. Sound Volume Liquid Slider: Speaker Icon + Crystal Azure Sloshing Fluid Column
    if el_type >= 4.9 && el_type <= 5.1 {
        // Speaker icon on the left
        let spk_center = vec2<f32>(-half.x + 24.0, 0.0);
        let spk_p = local_p - spk_center;
        
        // Trapezoid speaker body
        let d_body = max(abs(spk_p.y) - 6.0, abs(spk_p.x + 3.0) - 3.5);
        let d_cone = max(abs(spk_p.y) - (spk_p.x * 0.7 + 6.0), max(-spk_p.x - 3.0, spk_p.x - 4.5));
        let speaker = 1.0 - smoothstep(-0.6, 0.6, min(d_body, d_cone));
        
        // Radiating sound wave arcs
        let arc1 = abs(length(spk_p - vec2<f32>(0.0, 0.0)) - 9.0) - 1.4;
        let arc2 = abs(length(spk_p - vec2<f32>(0.0, 0.0)) - 14.5) - 1.4;
        let angle = atan2(abs(spk_p.y), spk_p.x);
        let in_sound_fan = select(0.0, 1.0, angle < 0.75);
        let sound_arcs = (1.0 - smoothstep(-0.6, 0.6, min(arc1, arc2))) * in_sound_fan;
        let spk_icon = max(speaker, sound_arcs);
        
        // Dynamic azure liquid fluid column
        let track_start = -half.x + 4.0;
        let track_width = half.x * 2.0 - 8.0;
        let fill_x = track_start + track_width * clamp(progress, 0.05, 0.98);
        let slosh = sin(local_p.y * 0.18 + time * 3.8) * 2.2;
        let meniscus_x = fill_x + slosh;
        
        let in_liquid = select(0.0, 1.0, local_p.x <= meniscus_x);
        let meniscus_rim = 1.0 - smoothstep(0.0, 2.5, abs(local_p.x - meniscus_x));
        
        app.glyph_mask = max(spk_icon * 0.95, meniscus_rim * 0.8);
        app.fluid_color = vec3<f32>(0.12, 0.75, 1.0); // Pure Crystal Azure
        app.fluid_glow = in_liquid * 0.38;
        return app;
    }
    
    // 6. Focus / Moon Liquid Lens: Apple Crescent Moon + Luminous Amethyst Core
    if el_type >= 5.9 && el_type <= 6.1 {
        let moon_outer = length(local_p) - 13.5;
        let moon_inner = length(local_p - vec2<f32>(6.0, -4.5)) - 11.5;
        let moon_d = max(moon_outer, -moon_inner);
        
        app.glyph_mask = 1.0 - smoothstep(-0.6, 0.6, moon_d);
        app.fluid_color = vec3<f32>(0.72, 0.38, 0.98); // Luminous Lavender/Amethyst
        app.fluid_glow = 0.48;
        return app;
    }
    
    // 7. Screen Mirroring Liquid Lens: Overlapping Display Screens + Radiant Sky Sapphire Core
    if el_type >= 6.9 && el_type <= 7.1 {
        // Front rectangle screen
        let scr1 = rr_sdf(local_p - vec2<f32>(-3.5, 2.5), vec2<f32>(11.0, 7.5), 2.5);
        let frame1 = abs(scr1) - 1.5;
        // Rear rectangle screen
        let scr2 = rr_sdf(local_p - vec2<f32>(3.5, -2.5), vec2<f32>(11.0, 7.5), 2.5);
        let frame2 = abs(scr2) - 1.5;
        
        let screens = min(frame1, frame2);
        app.glyph_mask = 1.0 - smoothstep(-0.6, 0.6, screens);
        app.fluid_color = vec3<f32>(0.15, 0.68, 1.0); // Sapphire/Sky
        app.fluid_glow = 0.48;
        return app;
    }
    
    // 8. Theme / Night Shift Liquid Lens: Dual Harmonized Sun/Moon + Amber Coral Core
    if el_type >= 7.9 && el_type <= 8.1 {
        let circle_d = length(local_p) - 11.0;
        let circle = 1.0 - smoothstep(-0.6, 0.6, circle_d);
        // Half-filled divider
        let left_half = select(0.0, 1.0, local_p.x < 0.0);
        let right_ring = 1.0 - smoothstep(-0.6, 0.6, abs(circle_d) - 1.6);
        let theme_glyph = max(circle * left_half, right_ring);
        
        app.glyph_mask = theme_glyph;
        app.fluid_color = vec3<f32>(1.0, 0.58, 0.28); // Sunset Amber Coral
        app.fluid_glow = 0.48;
        return app;
    }
    
    return app;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Compute pixel bounds
    let is_base_panel = (params.bounds.z >= 0.95 && params.bounds.w >= 0.95);
    let shadow_pad = select(8.0, 16.0, is_base_panel);
    
    var bounds_px: vec4<f32>;
    if is_base_panel {
        bounds_px = vec4<f32>(
            shadow_pad,
            shadow_pad,
            params.surface_size.x - shadow_pad * 2.0,
            params.surface_size.y - shadow_pad * 2.0
        );
    } else {
        bounds_px = vec4<f32>(
            params.bounds.x * params.surface_size.x,
            params.bounds.y * params.surface_size.y,
            params.bounds.z * params.surface_size.x,
            params.bounds.w * params.surface_size.y
        );
    }
    
    let half_size = vec2<f32>(bounds_px.z * 0.5, bounds_px.w * 0.5);
    let center = vec2<f32>(bounds_px.x + half_size.x, bounds_px.y + half_size.y);
    let local_px = uv * params.surface_size - center;
    let r = min(params.corner_radius, min(half_size.x, half_size.y));
    
    // ════════════════════════════════════════════
    // VISCOUS ELASTIC JELLY STRETCH (macOS Liquid Glass)
    // ════════════════════════════════════════════
    let cur_local = params.pointer_pos * params.surface_size - center;
    let d_to_cur = length(cur_local - local_px);
    let cur_active = select(0.0, 1.0, params.pointer_pos.x >= 0.0);
    
    // Stretch influence radius
    let stretch_radius = select(135.0, 210.0, is_base_panel);
    let stretch_t = clamp(1.0 - (d_to_cur / stretch_radius), 0.0, 1.0) * cur_active;
    let stretch_t2 = stretch_t * stretch_t;
    
    // 1. Magnetic adhesion pull: droplet contour reaches out toward the cursor (viscous mercury pull)
    let pull_dir = normalize(cur_local - local_px + vec2<f32>(0.001));
    let adhesion_pull = pull_dir * stretch_t2 * select(28.0, 14.0, is_base_panel);
    
    // 2. Velocity elongation: droplet stretches along cursor velocity vector (jelly shear)
    let vel_px = params.pointer_velocity * params.surface_size;
    let vel_len = length(vel_px);
    let vel_dir = select(vec2<f32>(0.0), normalize(vel_px), vel_len > 0.001);
    let jelly_elongation = vel_dir * min(vel_len * 0.85, 24.0) * stretch_t;
    
    // 3. Volume preservation pinch: droplet narrows along perpendicular axis when pulled
    let perp_dir = vec2<f32>(-pull_dir.y, pull_dir.x);
    let volume_pinch = perp_dir * dot(local_px, perp_dir) * (stretch_t2 * 0.12);
    
    // 4. Fluid wave displacement
    let liq = liquid_displacement(uv * params.surface_size);
    let liq_factor = select(0.008, 0.015, is_base_panel);
    
    // Net elastic liquid distortion (subtracting pulls contour outward toward cursor)
    let net_stretch = adhesion_pull + jelly_elongation - volume_pinch;
    let deformed_px = local_px - net_stretch + liq * liq_factor;
    
    let sdf = rr_sdf(deformed_px, half_size, r);
    
    // ════════════════════════════════════════════
    // 1. SOFT PHYSICAL CONTACT DROP SHADOW
    // ════════════════════════════════════════════
    let max_shadow_dist = select(10.0, shadow_pad, is_base_panel);
    if sdf > 0.0 {
        if sdf > max_shadow_dist {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
        let shadow_offset = vec2<f32>(0.0, select(2.0, 3.5, is_base_panel));
        let shadow_sdf = rr_sdf(deformed_px - shadow_offset, half_size, r);
        let d = max(shadow_sdf, 0.0);
        let spread = select(6.5, 13.0, is_base_panel);
        let outer_shadow = exp(-d * d / (spread * spread)) * select(0.32, 0.38, is_base_panel);
        let contact_shadow = exp(-d * 0.35) * select(0.28, 0.25, is_base_panel);
        let shadow = (outer_shadow + contact_shadow) * 0.5;
        let edge_fade = 1.0 - smoothstep(max_shadow_dist - 1.0, max_shadow_dist, d);
        return vec4<f32>(0.0, 0.0, 0.0, shadow * edge_fade);
    }
    
    // ════════════════════════════════════════════
    // 2. ANTI-ALIASED SILKY MASK
    // ════════════════════════════════════════════
    let mask = 1.0 - smoothstep(-1.2, 0.1, sdf);
    let inside = -sdf;
    let min_dim = min(half_size.x, half_size.y);
    let edge = smoothstep(min_dim * 0.4, 0.0, inside);
    
    // ════════════════════════════════════════════
    // 3. CONTINUOUS LIQUID DOME & SURFACE NORMAL
    // ════════════════════════════════════════════
    let z_radius = min(max(params.bevel, 3.5), min_dim * 0.85);
    let eps = 1.5;
    
    let h_c = droplet_height(deformed_px, half_size, r, z_radius);
    let h_r = droplet_height(deformed_px + vec2<f32>(eps, 0.0), half_size, r, z_radius);
    let h_l = droplet_height(deformed_px - vec2<f32>(eps, 0.0), half_size, r, z_radius);
    let h_u = droplet_height(deformed_px + vec2<f32>(0.0, eps), half_size, r, z_radius);
    let h_d = droplet_height(deformed_px - vec2<f32>(0.0, eps), half_size, r, z_radius);
    
    let h_grad = vec2<f32>(h_r - h_l, h_u - h_d) / (2.0 * eps);
    let tilt_3d = (cur_local / max(half_size, vec2<f32>(1.0))) * 0.18 * stretch_t;
    let wave_slope = liq * 0.003;
    let N = normalize(vec3<f32>(-(h_grad + wave_slope + tilt_3d), 1.0));
    
    // Optical thickness
    let thick_norm = clamp(h_c / max(z_radius, 1.0), 0.0, 1.0);
    
    // ════════════════════════════════════════════
    // 4. PLANO-CONVEX REFRACTION & LENS MAGNIFICATION
    // ════════════════════════════════════════════
    let px_to_uv = vec2<f32>(1.0, 1.0) / params.surface_size;
    let ior = params.ior;
    let refr_pow = 1.0 - 1.0 / ior;
    
    // Genuine optical magnification: contraction toward droplet center expands background image!
    let magnification_strength = select(0.18, 0.08, is_base_panel);
    let center_dir = -local_px / max(half_size, vec2<f32>(1.0));
    let lens_magnify = center_dir * magnification_strength * thick_norm * params.refraction_strength * 20.0;
    
    // Surface normal refraction ray displacement + stretch streaking
    let refr_ray = h_grad * refr_pow * params.refraction_strength * select(16.0, 24.0, is_base_panel);
    let lens_stretch = net_stretch * 0.38;
    let liq_advection = liq * 0.12;
    
    let refr_px = lens_magnify + refr_ray + liq_advection + lens_stretch;
    let refr = refr_px * px_to_uv;
    
    // ════════════════════════════════════════════
    // 5. CHROMATIC ABERRATION (Dispersion along curvature)
    // ════════════════════════════════════════════
    let ca_scale = params.dispersion * 16.0 * (edge * 0.8 + 0.2);
    let ca_offset = N.xy * ca_scale * px_to_uv;
    
    let base_uv = clamp(uv + refr, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_r = clamp(base_uv + ca_offset, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_g = base_uv;
    let uv_b = clamp(base_uv - ca_offset, vec2<f32>(0.001), vec2<f32>(0.999));
    
    // ════════════════════════════════════════════
    // 6. BACKGROUND SAMPLING (Sharp vs Vibrancy Blur)
    // ════════════════════════════════════════════
    let sharp_r = textureSampleLevel(background_texture, background_sampler, uv_r, 0.0).r;
    let sharp_g = textureSampleLevel(background_texture, background_sampler, uv_g, 0.0).g;
    let sharp_b = textureSampleLevel(background_texture, background_sampler, uv_b, 0.0).b;
    let sharp = vec3<f32>(sharp_r, sharp_g, sharp_b);
    
    let blur = params.blur_radius;
    let blur_r = blurred_sample(uv_r, blur).r;
    let blur_g = blurred_sample(uv_g, blur).g;
    let blur_b = blurred_sample(uv_b, blur).b;
    let blurred = vec3<f32>(blur_r, blur_g, blur_b);
    
    // Edge-weighted sharpness: center receives creamy blur, rim stays razor crisp
    let edge_mix = 1.0 - edge * 0.45;
    var col = mix(sharp, blurred, edge_mix);
    
    // ════════════════════════════════════════════
    // 7. SATURATION & CRYSTALLINE COLOR PURITY
    // ════════════════════════════════════════════
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    col = mix(vec3<f32>(lum), col, 1.10); // Rich 10% saturation lift
    col *= 1.0 + params.curvature * 0.05 * thick_norm; // Luminous volume lift
    
    if params.tint.a > 0.0 {
        col = mix(col, col * vec3<f32>(0.93, 0.96, 1.03), params.tint.a);
    }
    
    // ════════════════════════════════════════════
    // 8. DIRECTIONAL ILLUMINATION (No fake white outline!)
    // ════════════════════════════════════════════
    // Sunlight direction from top-left:
    let L_dir = normalize(vec2<f32>(-0.45, -0.7));
    let N_2d = N.xy;
    
    // 8a. Directional Meniscus Rim Highlight (Top/Left shoulder ONLY)
    let rim_alignment = max(dot(N_2d, L_dir), 0.0);
    let rim_crest = pow(edge, 1.8) * rim_alignment * 0.55 * params.specular_strength;
    
    // 8b. Ambient Occlusion / Internal Darkening (Bottom/Right edge)
    let shadow_alignment = max(dot(N_2d, -L_dir), 0.0);
    let edge_ao = pow(edge, 2.2) * shadow_alignment * 0.28;
    col *= (1.0 - edge_ao);
    
    // ════════════════════════════════════════════
    // 9. SHARP SPECULAR GLINT (Liquid Droplet Sheen)
    // ════════════════════════════════════════════
    let V = vec3<f32>(0.0, 0.0, 1.0);
    let L1 = normalize(vec3<f32>(-0.4, -0.65, 0.65));
    let H1 = normalize(L1 + V);
    let spec1 = pow(max(dot(N, H1), 0.0), 160.0) * 1.6 * params.specular_strength;
    
    // Softer sky fill reflection
    let L2 = normalize(vec3<f32>(0.3, -0.3, 0.9));
    let H2 = normalize(L2 + V);
    let spec2 = pow(max(dot(N, H2), 0.0), 36.0) * 0.22 * params.specular_strength;
    
    // ════════════════════════════════════════════
    // 9b. DYNAMIC CURSOR SPOTLIGHT (Apple Liquid Sheen)
    // ════════════════════════════════════════════
    let cur_pos_px = params.pointer_pos * params.surface_size;
    let delta_cur = cur_pos_px - (uv * params.surface_size);
    let dist_cur = length(delta_cur);
    let cur_radius = 150.0;
    
    var cur_spec = 0.0;
    var cur_rim_glow = 0.0;
    if dist_cur < cur_radius && params.pointer_pos.x >= 0.0 {
        let cur_t = 1.0 - (dist_cur / cur_radius);
        let cur_t2 = cur_t * cur_t;
        
        // Floating point light 35px above the surface
        let L_cur = normalize(vec3<f32>(delta_cur, 35.0));
        let H_cur = normalize(L_cur + V);
        cur_spec = pow(max(dot(N, H_cur), 0.0), 38.0) * cur_t2 * 1.25 * params.specular_strength;
        
        // Rim caustics oriented toward the cursor
        let cur_dir_2d = normalize(delta_cur + vec2<f32>(0.001));
        let rim_face_cur = max(dot(N_2d, cur_dir_2d), 0.0);
        cur_rim_glow = pow(edge, 1.8) * rim_face_cur * cur_t * 0.50;
    }
    
    // ════════════════════════════════════════════
    // 10. FRESNEL GRAZING-ANGLE REFLECTION
    // ════════════════════════════════════════════
    let cos_theta = clamp(abs(N.z), 0.0, 1.0);
    let fres = pow(1.0 - cos_theta, 4.0) * params.fresnel_strength;
    
    // ════════════════════════════════════════════
    // 11. AUTHENTIC MACOS LIQUID CORE & CRISP GLYPHS
    // ════════════════════════════════════════════
    let app = render_element_appearance(local_px, half_size, params.padding2.x, params.padding2.y, params.time);
    let glyph_crest = app.glyph_mask * vec3<f32>(1.0, 1.0, 1.0) * (1.15 + cur_spec * 0.6);
    let fluid_emission = app.fluid_color * app.fluid_glow;
    
    // ════════════════════════════════════════════
    // 12. MASTER COMPOSITE
    // ════════════════════════════════════════════
    var fin = col;
    // Infuse internal bioluminescent liquid fluid into the crystal body
    fin = mix(fin, fin * (1.0 + fluid_emission * 1.8) + fluid_emission * 0.45, app.fluid_glow);
    fin += vec3<f32>(rim_crest);
    fin += vec3<f32>(spec1 + spec2 + cur_spec + cur_rim_glow);
    fin += glyph_crest * 0.95;
    fin = mix(fin, vec3<f32>(1.0, 1.0, 1.0), fres * 0.12);
    
    return vec4<f32>(fin * mask, mask);
}
