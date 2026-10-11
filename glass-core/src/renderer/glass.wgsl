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

// ── Planar Frosted Glass with Curved Edge Bevel Profile ──
fn droplet_height(p: vec2<f32>, b: vec2<f32>, r: f32, z_radius: f32) -> f32 {
    let d = -rr_sdf(p, b, r);
    if d <= 0.0 {
        return 0.0;
    }
    
    // Apple Glass: flat planar body with a smooth, subtle curved perimeter bevel
    let meniscus_w = max(z_radius, 3.0);
    let rim_t = clamp(d / meniscus_w, 0.0, 1.0);
    return (1.0 - cos(rim_t * 1.5707963)) * meniscus_w;
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
    
    // 2. Wi-Fi Glass Capsule: Small, Professional Apple Wi-Fi Arcs (14px)
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
        
        app.glyph_mask = max(dot_mask, arcs_mask) * 0.72;
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
        app.glyph_mask = (1.0 - smoothstep(-0.4, 0.4, rune)) * 0.72;
        return app;
    }
    
    // 4. Display Brightness Liquid Slider: Delicate Sun Icon (12px) + Clean Hairline
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
        let sun_icon = max(sun_core, rays) * 0.72;
        
        let track_start = -half.x + 2.0;
        let track_width = half.x * 2.0 - 4.0;
        let fill_x = track_start + track_width * clamp(progress, 0.04, 0.98);
        
        let in_fill = select(0.0, 1.0, local_p.x <= fill_x);
        let divider = 1.0 - smoothstep(0.0, 1.0, abs(local_p.x - fill_x) - 0.4);
        
        app.glyph_mask = max(sun_icon, divider * 0.55);
        app.fluid_glow = in_fill * 0.08;
        return app;
    }
    
    // 5. Sound Volume Liquid Slider: Delicate Speaker Icon (12px) + Clean Hairline
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
        let spk_icon = max(speaker, sound_arcs) * 0.72;
        
        let track_start = -half.x + 2.0;
        let track_width = half.x * 2.0 - 4.0;
        let fill_x = track_start + track_width * clamp(progress, 0.04, 0.98);
        
        let in_fill = select(0.0, 1.0, local_p.x <= fill_x);
        let divider = 1.0 - smoothstep(0.0, 1.0, abs(local_p.x - fill_x) - 0.4);
        
        app.glyph_mask = max(spk_icon, divider * 0.55);
        app.fluid_glow = in_fill * 0.08;
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
    // Dynamic optical tilt towards cursor: creates responsive parallax without changing geometry
    let tilt_3d = (cur_local / max(half_size, vec2<f32>(1.0))) * 0.06 * stretch_t;
    let N = normalize(vec3<f32>(-(h_grad + tilt_3d), 1.0));
    
    // Optical thickness
    let thick_norm = clamp(h_c / max(z_radius, 1.0), 0.0, 1.0);
    
    // ════════════════════════════════════════════
    // 4. PLANO-CONVEX REFRACTION & LENS MAGNIFICATION
    // ════════════════════════════════════════════
    let px_to_uv = vec2<f32>(1.0, 1.0) / params.surface_size;
    let ior = params.ior;
    let refr_pow = 1.0 - 1.0 / ior;
    
    // Subtle optical magnification
    let magnification_strength = select(0.08, 0.04, is_base_panel);
    let center_dir = -local_px / max(half_size, vec2<f32>(1.0));
    let lens_magnify = center_dir * magnification_strength * thick_norm * params.refraction_strength * 10.0;
    
    // Surface normal refraction ray displacement
    let refr_ray = h_grad * refr_pow * params.refraction_strength * select(8.0, 14.0, is_base_panel);
    
    let refr_px = lens_magnify + refr_ray;
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
    
    // 8a. Subtle Directional Bevel Catch (Delicate, soft rim light)
    let rim_alignment = max(dot(N_2d, L_dir), 0.0);
    let rim_crest = pow(edge, 2.5) * rim_alignment * 0.22 * params.specular_strength;
    
    // 8b. Subtle Ambient Edge Softening
    let shadow_alignment = max(dot(N_2d, -L_dir), 0.0);
    let edge_ao = pow(edge, 3.0) * shadow_alignment * 0.06;
    col *= (1.0 - edge_ao);
    
    // ════════════════════════════════════════════
    // 9. SOFT DIFFUSE FROSTED SPECULAR
    // ════════════════════════════════════════════
    let V = vec3<f32>(0.0, 0.0, 1.0);
    let L1 = normalize(vec3<f32>(-0.4, -0.65, 0.7));
    let H1 = normalize(L1 + V);
    let spec1 = pow(max(dot(N, H1), 0.0), 28.0) * 0.25 * params.specular_strength;
    
    // Softer ambient sky sheen
    let L2 = normalize(vec3<f32>(0.3, -0.3, 0.9));
    let H2 = normalize(L2 + V);
    let spec2 = pow(max(dot(N, H2), 0.0), 16.0) * 0.10 * params.specular_strength;
    
    // ════════════════════════════════════════════
    // 9b. DYNAMIC CURSOR SPOTLIGHT (Apple Liquid Sheen)
    // ════════════════════════════════════════════
    let cur_pos_px = params.pointer_pos * params.surface_size;
    let delta_cur = cur_pos_px - (uv * params.surface_size);
    let dist_cur = length(delta_cur);
    let cur_radius = 140.0;
    
    var cur_spec = 0.0;
    var cur_rim_glow = 0.0;
    if dist_cur < cur_radius && params.pointer_pos.x >= 0.0 {
        let cur_t = 1.0 - (dist_cur / cur_radius);
        let cur_t2 = cur_t * cur_t;
        
        let L_cur = normalize(vec3<f32>(delta_cur, 35.0));
        let H_cur = normalize(L_cur + V);
        cur_spec = pow(max(dot(N, H_cur), 0.0), 24.0) * cur_t2 * 0.40 * params.specular_strength;
        
        let cur_dir_2d = normalize(delta_cur + vec2<f32>(0.001));
        let rim_face_cur = max(dot(N_2d, cur_dir_2d), 0.0);
        cur_rim_glow = pow(edge, 2.5) * rim_face_cur * cur_t * 0.22;
    }
    
    // ════════════════════════════════════════════
    // 10. FRESNEL GRAZING-ANGLE REFLECTION
    // ════════════════════════════════════════════
    let cos_theta = clamp(abs(N.z), 0.0, 1.0);
    let fres = pow(1.0 - cos_theta, 3.5) * params.fresnel_strength;
    
    // ════════════════════════════════════════════
    // 11. AUTHENTIC CRISP MINIMAL GLYPHS
    // ════════════════════════════════════════════
    let app = render_element_appearance(local_px, half_size, params.padding2.x, params.padding2.y, params.time);
    let glyph_crest = app.glyph_mask * vec3<f32>(1.0, 1.0, 1.0) * 0.95;
    let fluid_emission = app.fluid_color * app.fluid_glow;
    
    // ════════════════════════════════════════════
    // 12. MASTER COMPOSITE
    // ════════════════════════════════════════════
    var fin = col;
    // Infuse subtle internal crystal fluid tone
    fin = mix(fin, fin * (1.0 + fluid_emission * 1.2) + fluid_emission * 0.3, app.fluid_glow);
    fin += vec3<f32>(rim_crest);
    fin += vec3<f32>(spec1 + spec2 + cur_spec + cur_rim_glow);
    fin += glyph_crest;
    fin = mix(fin, vec3<f32>(1.0, 1.0, 1.0), fres * 0.08);
    
    return vec4<f32>(fin * mask, mask);
}
