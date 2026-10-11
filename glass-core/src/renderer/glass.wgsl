// Liquid Glass WGSL Shader
// High-fidelity physical simulation:
//   - Pill-shaped bevel HEIGHT FIELD (sqrt half-circle cross-section)
//   - Surface normal from height field gradient
//   - Biconvex dual-surface refraction (entry + exit)
//   - Chromatic aberration at the curved bevel rim
//   - 1.5px inner stroke highlight with top-bias rim lighting
//   - Multi-light Blinn-Phong specular highlights
//   - Physically calibrated Fresnel reflection
//   - Organic fluid pointer ripples and viscous advection
//   - Soft ambient contact shadow around rounded bounds

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
    padding2: vec2<f32>,
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

// ── Bevel Height Field ──
// Circular cross-section: h(d) = sqrt(d * (2*zR - d))
// Creates an optically true convex meniscus rim
fn bevel_height(d: f32, z_radius: f32) -> f32 {
    if d <= 0.0 { return 0.0; }
    if d >= z_radius { return z_radius; }
    return sqrt(d * (2.0 * z_radius - d));
}

// ── Micro-distortion noise hash ──
fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// ── 48-tap Vogel Golden Spiral Blur ──
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

// ── Fluid pointer ripple & viscous displacement ──
fn liquid_offset(world_px: vec2<f32>) -> vec2<f32> {
    var dp = vec2<f32>(0.0);
    
    // Continuous organic ambient wave
    if params.deformation > 0.0 {
        dp.x += sin(world_px.y * 0.012 + params.time * 1.4) * params.deformation * 6.0;
        dp.y += cos(world_px.x * 0.012 + params.time * 1.1) * params.deformation * 6.0;
    }
    
    // Interactive pointer wave & advection stretch
    let pointer_px = params.pointer_pos * params.surface_size;
    let dist = distance(world_px, pointer_px);
    let effect_radius = 180.0;
    
    if dist < effect_radius && length(params.pointer_velocity) > 0.0001 {
        let t = 1.0 - (dist / effect_radius);
        let t2 = t * t;
        let ripple = sin(dist * 0.06 - params.time * 12.0) * exp(-dist * 0.025);
        let vel = params.pointer_velocity * 25.0;
        let vel_mag = clamp(length(vel), 0.0, 1.0);
        
        let advection = vel * t2 * 14.0;
        let dir = normalize(world_px - pointer_px + vec2<f32>(0.001));
        let wave = dir * ripple * 10.0 * vel_mag * t;
        dp += advection + wave;
    }
    
    return dp;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Padding around the window so the drop shadow is not clipped
    let is_fullscreen_panel = (params.bounds.z >= 0.95 && params.bounds.w >= 0.95);
    let shadow_pad = select(8.0, 16.0, is_fullscreen_panel);
    
    var bounds_px: vec4<f32>;
    if is_fullscreen_panel {
        bounds_px = vec4<f32>(shadow_pad, shadow_pad, params.surface_size.x - shadow_pad * 2.0, params.surface_size.y - shadow_pad * 2.0);
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
    
    // Liquid displacement (stronger on large panels, subtle on subcomponents)
    let liq_scale = select(0.006, 0.015, is_fullscreen_panel);
    let liq = liquid_offset(uv * params.surface_size);
    let deformed_px = local_px + liq * liq_scale;
    
    let sdf = rr_sdf(deformed_px, half_size, r);
    
    // ════════════════════════════════════════════
    // 1. SOFT DROP SHADOW (Outside the glass surface)
    // ════════════════════════════════════════════
    let max_shadow_dist = select(10.0, shadow_pad, is_fullscreen_panel);
    if sdf > 0.0 {
        if sdf > max_shadow_dist {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
        let shadow_offset = vec2<f32>(0.0, select(2.0, 3.5, is_fullscreen_panel));
        let shadow_sdf = rr_sdf(local_px - shadow_offset, half_size, r);
        let d = max(shadow_sdf, 0.0);
        let spread = select(7.0, 14.0, is_fullscreen_panel);
        let outer_shadow = exp(-d * d / (spread * spread)) * select(0.30, 0.35, is_fullscreen_panel);
        let contact_shadow = exp(-d * 0.25) * select(0.22, 0.25, is_fullscreen_panel);
        let shadow = (outer_shadow + contact_shadow) * 0.45;
        let edge_fade = 1.0 - smoothstep(max_shadow_dist - 1.0, max_shadow_dist, d);
        return vec4<f32>(0.0, 0.0, 0.0, shadow * edge_fade);
    }
    
    // ════════════════════════════════════════════
    // 2. ANTI-ALIASED PANEL MASK
    // ════════════════════════════════════════════
    let mask = 1.0 - smoothstep(-1.2, 0.2, sdf);
    let inside = -sdf;
    let max_d = min(half_size.x, half_size.y);
    let edge = smoothstep(max_d * 0.35, 0.0, inside);
    
    // ════════════════════════════════════════════
    // 3. PILL BEVEL HEIGHT FIELD & SURFACE NORMAL
    // ════════════════════════════════════════════
    let z_radius = min(max(params.bevel, 3.0), min(half_size.x, half_size.y) * 0.85);
    let eps = 1.5;
    
    let d_c = inside;
    let d_r = -rr_sdf(deformed_px + vec2<f32>(eps, 0.0), half_size, r);
    let d_l = -rr_sdf(deformed_px - vec2<f32>(eps, 0.0), half_size, r);
    let d_u = -rr_sdf(deformed_px + vec2<f32>(0.0, eps), half_size, r);
    let d_d = -rr_sdf(deformed_px - vec2<f32>(0.0, eps), half_size, r);
    
    let h_c = bevel_height(d_c, z_radius);
    let h_r = bevel_height(d_r, z_radius);
    let h_l = bevel_height(d_l, z_radius);
    let h_u = bevel_height(d_u, z_radius);
    let h_d = bevel_height(d_d, z_radius);
    
    // Gradient of height field produces true physical surface normal
    let h_grad = vec2<f32>(h_r - h_l, h_u - h_d) / (2.0 * eps);
    let wave_slope = liq * 0.003;
    let N = normalize(vec3<f32>(-(h_grad + wave_slope), 1.0));
    
    let depth = smoothstep(0.0, z_radius, inside);
    
    // ════════════════════════════════════════════
    // 4. BICONVEX REFRACTION (Dual Surface Lensing)
    // ════════════════════════════════════════════
    let px_to_uv = vec2<f32>(1.0, 1.0) / params.surface_size;
    let ior = params.ior;
    let refr_pow = 1.0 - 1.0 / ior;
    let thickness = h_c * 2.0;
    let thick_norm = thickness / max(z_radius * 2.0, 1.0);
    
    // Refraction at entry and exit surfaces
    let exit_refr = h_grad * refr_pow;
    let entry_refr = h_grad * refr_pow;
    let through_refr = entry_refr * thick_norm * 0.5;
    var refr_px = (exit_refr + entry_refr + through_refr) * params.refraction_strength * 22.0;
    
    // Central optical magnification (lens dome effect)
    let center_dir = -local_px / max(half_size, vec2<f32>(1.0));
    refr_px += center_dir * params.refraction_strength * 3.5 * depth;
    
    // Fluid advection perturbation
    refr_px += liq * 0.12;
    let refr = refr_px * px_to_uv;
    
    // ════════════════════════════════════════════
    // 5. CHROMATIC ABERRATION
    // ════════════════════════════════════════════
    let ca_scale = params.dispersion * 14.0 * (edge * 0.7 + 0.3);
    let ca_offset = N.xy * ca_scale * px_to_uv;
    
    // Micro-surface roughness jitter
    let micro = (vec2<f32>(hash(local_px * 0.1), hash(local_px * 0.1 + vec2<f32>(43.0))) - 0.5)
              * params.roughness * 2.0 * px_to_uv;
              
    let base_uv = clamp(uv + refr + micro, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_r = clamp(base_uv + ca_offset, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_g = base_uv;
    let uv_b = clamp(base_uv - ca_offset, vec2<f32>(0.001), vec2<f32>(0.999));
    
    // ════════════════════════════════════════════
    // 6. BACKGROUND SAMPLING (Sharp vs Creamy Blur)
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
    
    // Edge-weighted sharpness: center is softly blurred, bevel rim retains crisp refraction
    let edge_mix = 1.0 - edge * 0.35;
    var col = mix(sharp, blurred, edge_mix);
    
    // ════════════════════════════════════════════
    // 7. MAC VIBRANCY & CRYSTALLINE POLISH
    // ════════════════════════════════════════════
    // Pure transmission: slight vibrance boost without whitening
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    col = mix(vec3<f32>(lum), col, 1.08); // 8% rich saturation boost
    col *= 1.0 + params.curvature * 0.04 * depth; // gentle luminous lift
    
    // Subtle cool crystalline glass tint
    if params.tint.a > 0.0 {
        col = mix(col, col * vec3<f32>(0.94, 0.97, 1.03), params.tint.a);
    }
    
    // ════════════════════════════════════════════
    // 8. FRESNEL GRAZING-ANGLE REFLECTION
    // ════════════════════════════════════════════
    let cos_theta = clamp(abs(N.z), 0.0, 1.0);
    let fres = pow(1.0 - cos_theta, 4.0) * params.fresnel_strength;
    
    // ════════════════════════════════════════════
    // 9. MULTI-LIGHT BLINN-PHONG SPECULAR
    // ════════════════════════════════════════════
    let V = vec3<f32>(0.0, 0.0, 1.0);
    
    // Overhead key light (sun/ceiling ambient)
    let L1 = normalize(vec3<f32>(0.35, -0.65, 1.0));
    let H1 = normalize(L1 + V);
    let sp1 = pow(max(dot(N, H1), 0.0), 96.0) * 0.55;
    
    // Fill rim light from opposite angle
    let L2 = normalize(vec3<f32>(-0.4, 0.5, 0.9));
    let H2 = normalize(L2 + V);
    let sp2 = pow(max(dot(N, H2), 0.0), 48.0) * 0.20;
    
    let total_spec = (sp1 + sp2) * params.specular_strength;
    
    // ════════════════════════════════════════════
    // 10. 1.5px INNER STROKE HIGHLIGHT (Mac Bevel Rim)
    // ════════════════════════════════════════════
    let border_width = 1.4;
    let inner_stroke = smoothstep(-border_width - 0.8, -border_width, sdf)
                     * (1.0 - smoothstep(-0.8, 0.0, sdf));
    // Light from top makes top-left edge gleam brighter
    let top_bias = 0.5 + 0.5 * (-local_px.y / half_size.y);
    let left_bias = 0.5 + 0.5 * (-local_px.x / half_size.x);
    let stroke_glow = inner_stroke * (0.3 + 0.45 * (top_bias * 0.7 + left_bias * 0.3));
    
    // ════════════════════════════════════════════
    // 11. EDGE MENISCUS HIGHLIGHT
    // ════════════════════════════════════════════
    let rim = edge * params.curvature * 0.12;
    let inner_glow = smoothstep(6.0, 0.0, inside) * params.curvature * 0.08;
    
    // ════════════════════════════════════════════
    // 12. FINAL COMPOSITING
    // ════════════════════════════════════════════
    var fin = col;
    fin += vec3<f32>(total_spec);
    fin += vec3<f32>(stroke_glow * 0.45);
    fin += vec3<f32>(rim + inner_glow);
    fin = mix(fin, vec3<f32>(1.0, 1.0, 1.0), fres * 0.12);
    
    // Premultiplied alpha output
    return vec4<f32>(fin * mask, mask);
}
