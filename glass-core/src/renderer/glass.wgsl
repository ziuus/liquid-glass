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

// ── Precision Apple Frosted Glass Profile (Planar Body + Micro Meniscus Edge) ──
fn droplet_height(p: vec2<f32>, b: vec2<f32>, r: f32, z_radius: f32) -> f32 {
    let d = -rr_sdf(p, b, r);
    if d <= 0.0 {
        return 0.0;
    }
    
    // Flat planar body with a razor-thin 3.5px perimeter bevel
    let meniscus_w = min(z_radius, 3.5);
    let rim_t = clamp(d / meniscus_w, 0.0, 1.0);
    return sin(rim_t * 1.5707963) * meniscus_w;
}

// ── 10-tap Optimized Golden Spiral Blur (Lightweight & Smooth) ──
fn blurred_sample(uv: vec2<f32>, radius: f32) -> vec3<f32> {
    if radius <= 0.5 {
        return textureSampleLevel(background_texture, background_sampler, uv, 0.0).rgb;
    }
    
    var color = vec3<f32>(0.0);
    let samples = 10.0;
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

// ── Struct for Clean Minimalist Element Appearance ──
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

// ── Clean Minimal Apple Liquid Glass Controls & Glyphs ──
fn render_element_appearance(local_p: vec2<f32>, half: vec2<f32>, el_type: f32, progress: f32, time: f32) -> ElementAppearance {
    var app: ElementAppearance;
    app.glyph_mask = 0.0;
    app.fluid_color = vec3<f32>(1.0, 1.0, 1.0);
    app.fluid_glow = 0.0;
    
    if el_type <= 0.0 {
        return app;
    }
    
    // 1. Dynamic Island Header Pill: Clean Minimal Status Capsule
    if el_type >= 0.9 && el_type <= 1.1 {
        let beacon_pos = local_p - vec2<f32>(-half.x + 14.0, 0.0);
        let beacon = 1.0 - smoothstep(-0.4, 0.4, length(beacon_pos) - 2.0);
        
        // Delicate minimal cellular signal bars
        let b1 = rr_sdf(local_p - vec2<f32>(half.x - 22.0, 1.8), vec2<f32>(0.9, 1.8), 0.4);
        let b2 = rr_sdf(local_p - vec2<f32>(half.x - 17.0, 0.8), vec2<f32>(0.9, 2.8), 0.4);
        let b3 = rr_sdf(local_p - vec2<f32>(half.x - 12.0, -0.4), vec2<f32>(0.9, 4.0), 0.4);
        let bars = 1.0 - smoothstep(-0.4, 0.4, min(b1, min(b2, b3)));
        
        app.glyph_mask = max(beacon * 0.80, bars * 0.65);
        app.fluid_color = mix(vec3<f32>(0.35, 0.90, 0.60), vec3<f32>(1.0), bars);
        app.fluid_glow = beacon * 0.25;
        return app;
    }
    
    // 2. Wi-Fi Glass Capsule: Small, Professional Apple Wi-Fi Arcs (14px) - Active Apple Blue
    if el_type >= 1.9 && el_type <= 2.1 {
        let p_rel = local_p - vec2<f32>(0.0, 4.5);
        let r = length(p_rel);
        
        let dot_mask = 1.0 - smoothstep(-0.4, 0.4, r - 1.5);
        
        let arc1 = abs(r - 5.0) - 0.75;
        let arc2 = abs(r - 8.5) - 0.75;
        let arc3 = abs(r - 12.0) - 0.75;
        let min_arc = min(arc1, min(arc2, arc3));
        
        let angle = atan2(p_rel.y, abs(p_rel.x));
        let in_wedge = select(0.0, 1.0, angle < -0.85);
        let arcs_mask = (1.0 - smoothstep(-0.4, 0.4, min_arc)) * in_wedge;
        
        app.glyph_mask = max(dot_mask, arcs_mask) * 0.95;
        app.fluid_color = vec3<f32>(0.0, 0.48, 1.0); // Vibrant Apple Blue #007AFF
        app.fluid_glow = 0.88;
        return app;
    }
    
    // 3. Bluetooth Glass Capsule: Small, Professional Apple Bluetooth (14px)
    if el_type >= 2.9 && el_type <= 3.1 {
        let d_spine = dist_segment(local_p, vec2<f32>(0.0, -7.5), vec2<f32>(0.0, 7.5)) - 0.8;
        let d_up1 = dist_segment(local_p, vec2<f32>(0.0, -7.5), vec2<f32>(4.5, -3.8)) - 0.8;
        let d_up2 = dist_segment(local_p, vec2<f32>(4.5, -3.8), vec2<f32>(-4.0, 3.8)) - 0.8;
        let d_dn1 = dist_segment(local_p, vec2<f32>(0.0, 7.5), vec2<f32>(4.5, 3.8)) - 0.8;
        let d_dn2 = dist_segment(local_p, vec2<f32>(4.5, 3.8), vec2<f32>(-4.0, -3.8)) - 0.8;
        
        let rune = min(d_spine, min(min(d_up1, d_up2), min(d_dn1, d_dn2)));
        app.glyph_mask = (1.0 - smoothstep(-0.4, 0.4, rune)) * 0.90;
        return app;
    }
    
    // 4. Display Brightness Liquid Slider: Delicate Sun Icon (12px) + Solid Luminous Fill
    if el_type >= 3.9 && el_type <= 4.1 {
        let sun_center = vec2<f32>(-half.x + 16.0, 0.0);
        let sun_p = local_p - sun_center;
        let sun_r = length(sun_p);
        let sun_core = 1.0 - smoothstep(-0.4, 0.4, sun_r - 2.5);
        
        var rays = 0.0;
        for (var i = 0.0; i < 8.0; i += 1.0) {
            let theta = i * 0.785398;
            let ray_start = vec2<f32>(cos(theta), sin(theta)) * 4.0;
            let ray_end = vec2<f32>(cos(theta), sin(theta)) * 5.8;
            let d_ray = dist_segment(sun_p, ray_start, ray_end) - 0.65;
            rays = max(rays, 1.0 - smoothstep(-0.4, 0.4, d_ray));
        }
        let sun_icon = max(sun_core, rays) * 0.90;
        
        let track_start = -half.x + 2.0;
        let track_width = half.x * 2.0 - 4.0;
        let fill_x = track_start + track_width * clamp(progress, 0.04, 0.98);
        
        let in_fill = select(0.0, 1.0, local_p.x <= fill_x);
        let divider = 1.0 - smoothstep(0.0, 1.0, abs(local_p.x - fill_x) - 0.5);
        
        app.glyph_mask = max(sun_icon, divider * 0.70);
        app.fluid_color = vec3<f32>(0.92, 0.94, 0.98);
        app.fluid_glow = in_fill * 0.85;
        return app;
    }
    
    // 5. Sound Volume Liquid Slider: Delicate Speaker Icon (12px) + Solid Luminous Fill
    if el_type >= 4.9 && el_type <= 5.1 {
        let spk_center = vec2<f32>(-half.x + 16.0, 0.0);
        let spk_p = local_p - spk_center;
        
        let d_body = max(abs(spk_p.y) - 2.8, abs(spk_p.x + 2.0) - 1.8);
        let d_cone = max(abs(spk_p.y) - (spk_p.x * 0.7 + 2.8), max(-spk_p.x - 2.0, spk_p.x - 2.5));
        let speaker = 1.0 - smoothstep(-0.4, 0.4, min(d_body, d_cone));
        
        let arc1 = abs(length(spk_p) - 5.0) - 0.75;
        let arc2 = abs(length(spk_p) - 8.0) - 0.75;
        let angle = atan2(abs(spk_p.y), spk_p.x);
        let in_sound_fan = select(0.0, 1.0, angle < 0.70);
        let sound_arcs = (1.0 - smoothstep(-0.4, 0.4, min(arc1, arc2))) * in_sound_fan;
        let spk_icon = max(speaker, sound_arcs) * 0.90;
        
        let track_start = -half.x + 2.0;
        let track_width = half.x * 2.0 - 4.0;
        let fill_x = track_start + track_width * clamp(progress, 0.04, 0.98);
        
        let in_fill = select(0.0, 1.0, local_p.x <= fill_x);
        let divider = 1.0 - smoothstep(0.0, 1.0, abs(local_p.x - fill_x) - 0.5);
        
        app.glyph_mask = max(spk_icon, divider * 0.70);
        app.fluid_color = vec3<f32>(0.92, 0.94, 0.98);
        app.fluid_glow = in_fill * 0.85;
        return app;
    }
    
    // 6. Focus / Moon Lens: Delicate Crescent Moon (13px)
    if el_type >= 5.9 && el_type <= 6.1 {
        let moon_outer = length(local_p) - 6.5;
        let moon_inner = length(local_p - vec2<f32>(2.8, -2.2)) - 5.5;
        let moon_d = max(moon_outer, -moon_inner);
        
        app.glyph_mask = (1.0 - smoothstep(-0.4, 0.4, moon_d)) * 0.72;
        return app;
    }
    
    // 7. Screen Mirroring Lens: Delicate Dual Screens (12px)
    if el_type >= 6.9 && el_type <= 7.1 {
        let scr1 = rr_sdf(local_p - vec2<f32>(-2.0, 1.5), vec2<f32>(5.5, 3.8), 1.0);
        let frame1 = abs(scr1) - 0.8;
        let scr2 = rr_sdf(local_p - vec2<f32>(2.0, -1.5), vec2<f32>(5.5, 3.8), 1.0);
        let frame2 = abs(scr2) - 0.8;
        
        app.glyph_mask = (1.0 - smoothstep(-0.4, 0.4, min(frame1, frame2))) * 0.72;
        return app;
    }
    
    // 8. Theme / Night Shift Lens: Delicate Sun/Moon Disc (12px)
    if el_type >= 7.9 && el_type <= 8.1 {
        let circle_d = length(local_p) - 6.0;
        let circle = 1.0 - smoothstep(-0.4, 0.4, circle_d);
        let left_half = select(0.0, 1.0, local_p.x < 0.0);
        let right_ring = 1.0 - smoothstep(-0.4, 0.4, abs(circle_d) - 0.8);
        
        app.glyph_mask = max(circle * left_half, right_ring) * 0.72;
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
    // CLEAN GEOMETRIC CONTOUR & INTERACTIVE DYNAMICS
    // ════════════════════════════════════════════
    let cur_local = params.pointer_pos * params.surface_size - center;
    let d_to_cur = length(cur_local - local_px);
    let cur_active = select(0.0, 1.0, params.pointer_pos.x >= 0.0);
    
    // Subtle proximity influence for optical tilt and light interaction
    let stretch_radius = select(110.0, 180.0, is_base_panel);
    let stretch_t = clamp(1.0 - (d_to_cur / stretch_radius), 0.0, 1.0) * cur_active;
    
    // Crisp, stable rounded-rectangle SDF (no teardrop egg distortion)
    let sdf = rr_sdf(local_px, half_size, r);
    
    // ════════════════════════════════════════════
    // 1. SOFT PHYSICAL CONTACT DROP SHADOW
    // ════════════════════════════════════════════
    let max_shadow_dist = select(8.0, shadow_pad, is_base_panel);
    if sdf > 0.0 {
        if sdf > max_shadow_dist {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
        let shadow_offset = vec2<f32>(0.0, select(1.5, 3.0, is_base_panel));
        let shadow_sdf = rr_sdf(local_px - shadow_offset, half_size, r);
        let d = max(shadow_sdf, 0.0);
        let spread = select(5.5, 12.0, is_base_panel);
        let outer_shadow = exp(-d * d / (spread * spread)) * select(0.24, 0.32, is_base_panel);
        let contact_shadow = exp(-d * 0.40) * select(0.22, 0.22, is_base_panel);
        let shadow = (outer_shadow + contact_shadow) * 0.5;
        let edge_fade = 1.0 - smoothstep(max_shadow_dist - 1.0, max_shadow_dist, d);
        return vec4<f32>(0.0, 0.0, 0.0, shadow * edge_fade);
    }
    
    // ════════════════════════════════════════════
    // 2. ANTI-ALIASED SILKY MASK
    // ════════════════════════════════════════════
    let mask = 1.0 - smoothstep(-1.0, 0.1, sdf);
    let inside = -sdf;
    let min_dim = min(half_size.x, half_size.y);
    let edge = smoothstep(min_dim * 0.35, 0.0, inside);
    
    // ════════════════════════════════════════════
    // 3. CONTINUOUS LIQUID DOME & SURFACE NORMAL
    // ════════════════════════════════════════════
    let z_radius = min(max(params.bevel, 2.5), min_dim * 0.75);
    let eps = 1.0;
    
    let h_c = droplet_height(local_px, half_size, r, z_radius);
    let h_r = droplet_height(local_px + vec2<f32>(eps, 0.0), half_size, r, z_radius);
    let h_l = droplet_height(local_px - vec2<f32>(eps, 0.0), half_size, r, z_radius);
    let h_u = droplet_height(local_px + vec2<f32>(0.0, eps), half_size, r, z_radius);
    let h_d = droplet_height(local_px - vec2<f32>(0.0, eps), half_size, r, z_radius);
    
    let h_grad = vec2<f32>(h_r - h_l, h_u - h_d) / (2.0 * eps);
    // Subtle optical parallax tilt towards cursor
    let tilt_3d = (cur_local / max(half_size, vec2<f32>(1.0))) * 0.04 * stretch_t;
    let N = normalize(vec3<f32>(-(h_grad * 1.5 + tilt_3d), 1.0));
    
    // ════════════════════════════════════════════
    // 4. PRECISION BORDER REFRACTION (No bubble bulging)
    // ════════════════════════════════════════════
    let px_to_uv = vec2<f32>(1.0, 1.0) / params.surface_size;
    let ior = params.ior;
    let refr_pow = 1.0 - 1.0 / ior;
    
    // Subtle 3.5px perimeter meniscus refraction only at outer boundary of base panel
    let refr_scale = select(0.0, 3.5, is_base_panel);
    let refr_ray = N.xy * refr_pow * refr_scale;
    let refr = refr_ray * px_to_uv;
    
    let ca_scale = select(0.0, params.dispersion * 8.0 * edge, is_base_panel);
    let ca_offset = N.xy * ca_scale * px_to_uv;
    
    let uv_r = clamp(uv + refr + ca_offset, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_g = clamp(uv + refr, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_b = clamp(uv + refr - ca_offset, vec2<f32>(0.001), vec2<f32>(0.999));
    
    // ════════════════════════════════════════════
    // 5. BACKGROUND SAMPLING & ACRYLIC MATERIAL
    // ════════════════════════════════════════════
    let blur = params.blur_radius;
    let blur_r = blurred_sample(uv_r, blur).r;
    let blur_g = blurred_sample(uv_g, blur).g;
    let blur_b = blurred_sample(uv_b, blur).b;
    let blurred = vec3<f32>(blur_r, blur_g, blur_b);
    
    var col = blurred;
    
    if is_base_panel {
        // Authentic macOS Sonoma Dark Frosted Acrylic Window:
        // Deep smoked obsidian glass that harmonizes with any wallpaper
        let dark_acrylic = vec3<f32>(0.07, 0.08, 0.11);
        let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
        let saturated = mix(vec3<f32>(lum), col, 1.30); // 30% saturation lift
        col = mix(saturated * 0.65, dark_acrylic, 0.64);
    } else {
        // Subcomponent plate on the acrylic canvas:
        let plate_tint = params.tint.rgb;
        let plate_alpha = params.tint.a;
        col = mix(col, plate_tint, plate_alpha);
    }
    
    // ════════════════════════════════════════════
    // 6. PRECISION 1PX BORDER & INNER RIM LIGHT
    // ════════════════════════════════════════════
    let border_1px = smoothstep(1.0, 0.0, inside);
    let L_dir = normalize(vec2<f32>(-0.45, -0.7));
    let N_2d = N.xy;
    let border_light = max(dot(N_2d, L_dir), 0.0) * 0.20 + select(0.06, 0.12, is_base_panel);
    let border_crest = border_1px * border_light * params.specular_strength;
    
    // Delicate top-edge inner catch
    let inner_rim = smoothstep(1.0, 0.0, abs(inside - 1.5)) * max(-N_2d.y, 0.0) * 0.10 * params.specular_strength;
    
    // ════════════════════════════════════════════
    // 7. INTERACTIVE FLUID LIGHT (Cursor Sheen)
    // ════════════════════════════════════════════
    let cur_pos_px = params.pointer_pos * params.surface_size;
    let delta_cur = cur_pos_px - (uv * params.surface_size);
    let dist_cur = length(delta_cur);
    let cur_radius = 80.0;
    
    var cur_glow = 0.0;
    if dist_cur < cur_radius && params.pointer_pos.x >= 0.0 {
        let cur_t = 1.0 - (dist_cur / cur_radius);
        cur_glow = cur_t * cur_t * 0.15 * params.specular_strength;
    }
    
    // ════════════════════════════════════════════
    // 8. CONTROLS, FLUID EMISSION & GLYPHS
    // ════════════════════════════════════════════
    let app = render_element_appearance(local_px, half_size, params.padding2.x, params.padding2.y, params.time);
    
    if app.fluid_glow > 0.0 {
        col = mix(col, app.fluid_color, app.fluid_glow);
    }
    
    // Invert slider icon to dark charcoal over white fill for perfect contrast
    var glyph_color = vec3<f32>(1.0, 1.0, 1.0);
    if (params.padding2.x >= 3.9 && params.padding2.x <= 5.1) {
        let track_start = -half_size.x + 2.0;
        let track_width = half_size.x * 2.0 - 4.0;
        let fill_x = track_start + track_width * clamp(params.padding2.y, 0.04, 0.98);
        if local_px.x <= fill_x {
            glyph_color = vec3<f32>(0.12, 0.14, 0.18);
        }
    }
    
    // ════════════════════════════════════════════
    // 9. MASTER COMPOSITE
    // ════════════════════════════════════════════
    var fin = col;
    fin += vec3<f32>(border_crest + inner_rim + cur_glow);
    fin = mix(fin, glyph_color, app.glyph_mask);
    
    return vec4<f32>(fin * mask, mask);
}
