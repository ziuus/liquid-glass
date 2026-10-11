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

// ── Authentic Apple Liquid Glass Profile (Fluid Meniscus + Optical Lens) ──
fn droplet_height(p: vec2<f32>, b: vec2<f32>, r: f32, z_radius: f32) -> f32 {
    let d = -rr_sdf(p, b, r);
    if d <= 0.0 {
        return 0.0;
    }
    
    // 1. Capillary Surface Tension Meniscus (steep curve at perimeter, smooth roll-off)
    let meniscus_w = min(z_radius, 6.0);
    let rim_t = clamp(d / meniscus_w, 0.0, 1.0);
    let meniscus = 1.0 - pow(1.0 - rim_t, 2.2);
    
    // 2. Continuous Fluid Volume (subtle lens curve across interior)
    let min_d = min(b.x, b.y);
    let norm_d = clamp(d / max(min_d, 1.0), 0.0, 1.0);
    let fluid_lens = sin(norm_d * 1.5707963) * 0.16;
    
    return (meniscus * 0.84 + fluid_lens) * z_radius;
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
    glyph_color: vec3<f32>,
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

// ── Precision Vector Letter SDFs for Authentic Camera Toolbar Typography ──
fn draw_letter(p: vec2<f32>, ch: u32, scale: f32) -> f32 {
    let q = p / scale;
    var d = 999.0;
    switch ch {
        // 'C'
        case 67u: {
            let ang = atan2(q.y, q.x);
            if abs(ang) < 0.85 {
                let end_top = vec2<f32>(3.4 * cos(0.85), -3.4 * sin(0.85));
                let end_bot = vec2<f32>(3.4 * cos(0.85),  3.4 * sin(0.85));
                d = min(length(q - end_top), length(q - end_bot));
            } else {
                d = abs(length(q) - 3.4);
            }
        }
        // 'I'
        case 73u: {
            d = dist_segment(q, vec2<f32>(0.0, -4.2), vec2<f32>(0.0, 4.2));
        }
        // 'N'
        case 78u: {
            d = min(dist_segment(q, vec2<f32>(-2.6, -4.2), vec2<f32>(-2.6, 4.2)),
                min(dist_segment(q, vec2<f32>(-2.6, -4.2), vec2<f32>(2.6, 4.2)),
                    dist_segment(q, vec2<f32>(2.6, -4.2), vec2<f32>(2.6, 4.2))));
        }
        // 'E'
        case 69u: {
            d = min(dist_segment(q, vec2<f32>(-2.4, -4.2), vec2<f32>(-2.4, 4.2)),
                min(dist_segment(q, vec2<f32>(-2.4, -4.2), vec2<f32>(2.2, -4.2)),
                min(dist_segment(q, vec2<f32>(-2.4, 0.0),  vec2<f32>(1.6, 0.0)),
                    dist_segment(q, vec2<f32>(-2.4, 4.2),  vec2<f32>(2.2, 4.2)))));
        }
        // 'M'
        case 77u: {
            d = min(dist_segment(q, vec2<f32>(-3.2, 4.2), vec2<f32>(-3.2, -4.2)),
                min(dist_segment(q, vec2<f32>(-3.2, -4.2), vec2<f32>(0.0, 0.5)),
                min(dist_segment(q, vec2<f32>(0.0, 0.5), vec2<f32>(3.2, -4.2)),
                    dist_segment(q, vec2<f32>(3.2, -4.2), vec2<f32>(3.2, 4.2)))));
        }
        // 'A'
        case 65u: {
            d = min(dist_segment(q, vec2<f32>(-2.6, 4.2), vec2<f32>(0.0, -4.2)),
                min(dist_segment(q, vec2<f32>(0.0, -4.2), vec2<f32>(2.6, 4.2)),
                    dist_segment(q, vec2<f32>(-1.4, 1.0), vec2<f32>(1.4, 1.0))));
        }
        // 'T'
        case 84u: {
            d = min(dist_segment(q, vec2<f32>(-2.8, -4.2), vec2<f32>(2.8, -4.2)),
                    dist_segment(q, vec2<f32>(0.0, -4.2), vec2<f32>(0.0, 4.2)));
        }
        // 'P'
        case 80u: {
            let stem = dist_segment(q, vec2<f32>(-2.4, 4.2), vec2<f32>(-2.4, -4.2));
            let loop_top = dist_segment(q, vec2<f32>(-2.4, -4.2), vec2<f32>(0.8, -4.2));
            let loop_bot = dist_segment(q, vec2<f32>(-2.4, -0.2), vec2<f32>(0.8, -0.2));
            let loop_side = dist_segment(q, vec2<f32>(0.8, -4.2), vec2<f32>(0.8, -0.2));
            d = min(stem, min(loop_top, min(loop_bot, loop_side)));
        }
        // 'H'
        case 72u: {
            d = min(dist_segment(q, vec2<f32>(-2.4, -4.2), vec2<f32>(-2.4, 4.2)),
                min(dist_segment(q, vec2<f32>(2.4, -4.2), vec2<f32>(2.4, 4.2)),
                    dist_segment(q, vec2<f32>(-2.4, 0.0), vec2<f32>(2.4, 0.0))));
        }
        // 'O'
        case 79u: {
            d = abs(length(q) - 3.4);
        }
        // 'V'
        case 86u: {
            d = min(dist_segment(q, vec2<f32>(-2.6, -4.2), vec2<f32>(0.0, 4.2)),
                    dist_segment(q, vec2<f32>(0.0, 4.2), vec2<f32>(2.6, -4.2)));
        }
        // 'D'
        case 68u: {
            let stem = dist_segment(q, vec2<f32>(-2.4, -4.2), vec2<f32>(-2.4, 4.2));
            let top_b = dist_segment(q, vec2<f32>(-2.4, -4.2), vec2<f32>(0.4, -4.2));
            let bot_b = dist_segment(q, vec2<f32>(-2.4, 4.2), vec2<f32>(0.4, 4.2));
            let arc_b = abs(length(q - vec2<f32>(0.4, 0.0)) - 4.2);
            let curve = select(999.0, arc_b, q.x >= 0.4);
            d = min(stem, min(top_b, min(bot_b, curve)));
        }
        default: {}
    }
    return d * scale;
}

fn draw_word_cinematic(p: vec2<f32>, scale: f32) -> f32 {
    let sp = 8.4 * scale;
    let d0 = draw_letter(p - vec2<f32>(-4.0 * sp, 0.0), 67u, scale);
    let d1 = draw_letter(p - vec2<f32>(-3.0 * sp, 0.0), 73u, scale);
    let d2 = draw_letter(p - vec2<f32>(-2.0 * sp, 0.0), 78u, scale);
    let d3 = draw_letter(p - vec2<f32>(-1.0 * sp, 0.0), 69u, scale);
    let d4 = draw_letter(p - vec2<f32>( 0.0 * sp, 0.0), 77u, scale);
    let d5 = draw_letter(p - vec2<f32>( 1.0 * sp, 0.0), 65u, scale);
    let d6 = draw_letter(p - vec2<f32>( 2.0 * sp, 0.0), 84u, scale);
    let d7 = draw_letter(p - vec2<f32>( 3.0 * sp, 0.0), 73u, scale);
    let d8 = draw_letter(p - vec2<f32>( 4.0 * sp, 0.0), 67u, scale);
    return min(d0, min(d1, min(d2, min(d3, min(d4, min(d5, min(d6, min(d7, d8))))))));
}

fn draw_word_photo(p: vec2<f32>, scale: f32) -> f32 {
    let sp = 8.2 * scale;
    let d0 = draw_letter(p - vec2<f32>(-2.0 * sp, 0.0), 80u, scale);
    let d1 = draw_letter(p - vec2<f32>(-1.0 * sp, 0.0), 72u, scale);
    let d2 = draw_letter(p - vec2<f32>( 0.0 * sp, 0.0), 79u, scale);
    let d3 = draw_letter(p - vec2<f32>( 1.0 * sp, 0.0), 84u, scale);
    let d4 = draw_letter(p - vec2<f32>( 2.0 * sp, 0.0), 79u, scale);
    return min(d0, min(d1, min(d2, min(d3, d4))));
}

fn draw_word_video(p: vec2<f32>, scale: f32) -> f32 {
    let sp = 8.2 * scale;
    let d0 = draw_letter(p - vec2<f32>(-2.0 * sp, 0.0), 86u, scale);
    let d1 = draw_letter(p - vec2<f32>(-1.0 * sp, 0.0), 73u, scale);
    let d2 = draw_letter(p - vec2<f32>( 0.0 * sp, 0.0), 68u, scale);
    let d3 = draw_letter(p - vec2<f32>( 1.0 * sp, 0.0), 69u, scale);
    let d4 = draw_letter(p - vec2<f32>( 2.0 * sp, 0.0), 79u, scale);
    return min(d0, min(d1, min(d2, min(d3, d4))));
}

fn dist_bent_arrow(p: vec2<f32>) -> f32 {
    let s = 1.45;
    let q = p / s;
    // Arrow top: horizontal shaft to the left from x = 1.8 to x = -6.0 at y = -4.5
    let h_shaft = dist_segment(q, vec2<f32>(-6.0, -4.5), vec2<f32>(1.8, -4.5));
    // Vertical shaft on the right from (4.8, -1.5) to (4.8, 1.8)
    let v_shaft = dist_segment(q, vec2<f32>(4.8, -1.5), vec2<f32>(4.8, 1.8));
    // 90 deg corner arc between (1.8, -4.5) and (4.8, -1.5) centered at (1.8, -1.5) with radius 3.0
    var corner = 999.0;
    if q.x >= 1.8 && q.y <= -1.5 {
        corner = abs(length(q - vec2<f32>(1.8, -1.5)) - 3.0);
    }
    // Arrowhead at (-6.0, -4.5) pointing left
    let barb_top = dist_segment(q, vec2<f32>(-6.0, -4.5), vec2<f32>(-2.8, -7.8));
    let barb_bot = dist_segment(q, vec2<f32>(-6.0, -4.5), vec2<f32>(-2.8, -1.2));
    return min(min(h_shaft, v_shaft), min(corner, min(barb_top, barb_bot))) * s;
}

// ── Clean Minimal Liquid Glass Camera Toolbar Elements ──
fn render_element_appearance(local_p: vec2<f32>, half: vec2<f32>, el_type: f32, progress: f32, time: f32) -> ElementAppearance {
    var app: ElementAppearance;
    app.glyph_mask = 0.0;
    app.glyph_color = vec3<f32>(1.0, 1.0, 1.0);
    app.fluid_color = vec3<f32>(1.0, 1.0, 1.0);
    app.fluid_glow = 0.0;
    
    if el_type <= 0.0 {
        return app;
    }
    
    // 10. Left Element: Circular Camera Gallery Preview Disc
    if el_type >= 9.9 && el_type <= 10.1 {
        let r_p = length(local_p);
        let ring = abs(r_p - 26.5) - 0.55;
        let ring_mask = (1.0 - smoothstep(-0.35, 0.35, ring)) * 0.45;
        app.glyph_mask = ring_mask;
        app.glyph_color = vec3<f32>(0.98, 0.99, 1.0);
        return app;
    }
    
    // 11. Center Element: Outer Glass Track Capsule (Serene, clear liquid glass)
    if el_type >= 10.9 && el_type <= 11.1 {
        // Track has no text - pristine clear glass
        return app;
    }
    
    // 12. Active Sliding Liquid Glass Mode Pill: Radiant Mode Name in neon gold #DCF842
    if el_type >= 11.9 && el_type <= 12.1 {
        let text_scale = 0.84;
        var d_text = 999.0;
        if progress < 0.5 {
            d_text = draw_word_photo(local_p, text_scale);
        } else if progress < 1.5 {
            d_text = draw_word_cinematic(local_p, text_scale);
        } else {
            d_text = draw_word_video(local_p, text_scale);
        }
        
        let stroke_mask = 1.0 - smoothstep(-0.35, 0.35, d_text - 0.58);
        app.glyph_mask = stroke_mask * 0.98;
        // Signature luminous liquid gold #DCF842 from liquidglassdesign.com
        let gold = vec3<f32>(0.86, 0.97, 0.26);
        app.glyph_color = gold;
        
        // Soft radiant gold bloom around the active text (additive emission)
        let text_glow = exp(-max(d_text, 0.0) * 0.65) * 0.45;
        app.fluid_color = gold;
        app.fluid_glow = text_glow;
        return app;
    }
    
    // 13. Right Element: Circular Switch Camera Button (Exact bent loop arrows)
    if el_type >= 12.9 && el_type <= 13.1 {
        let d_arrow = min(dist_bent_arrow(local_p), dist_bent_arrow(-local_p)) - 0.72;
        let icon = 1.0 - smoothstep(-0.35, 0.35, d_arrow);
        
        app.glyph_mask = icon * 0.98;
        app.glyph_color = vec3<f32>(0.98, 0.99, 1.0);
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
    let cur_active = select(0.0, 1.0, params.pointer_pos.x >= 0.0);
    
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
    
    // Reactive fluid pull: liquid surface tension draws towards pointer
    let cur_vec = cur_local - local_px;
    let dist_cur = length(cur_vec);
    let cur_dir = cur_vec / max(dist_cur, 0.001);
    let stretch_radius = select(100.0, 160.0, is_base_panel);
    let pull_t = clamp(1.0 - (dist_cur / stretch_radius), 0.0, 1.0) * cur_active;
    let fluid_tilt = cur_dir * (pull_t * pull_t) * 0.08;
    
    let N = normalize(vec3<f32>(-(h_grad * params.curvature * 2.2 + fluid_tilt), 1.0));
    
    // ════════════════════════════════════════════
    // 4. TRUE OPTICAL REFRACTION & FLUID STRETCH (Snell's Law)
    // ════════════════════════════════════════════
    let px_to_uv = vec2<f32>(1.0, 1.0) / params.surface_size;
    let ior = params.ior;
    let refr_pow = 1.0 - 1.0 / ior;
    
    // Optical ray bending through liquid glass
    let refr_scale = select(14.0, 18.0, is_base_panel) * params.refraction_strength;
    let refr_ray = N.xy * refr_pow * refr_scale;
    
    // Reactive background stretch: wallpaper behind glass is pulled toward cursor
    let fluid_stretch = cur_dir * pow(pull_t, 1.5) * select(12.0, 16.0, is_base_panel) * params.refraction_strength;
    
    let total_refr_px = refr_ray + fluid_stretch;
    let refr = total_refr_px * px_to_uv;
    
    // Chromatic dispersion (prismatic rainbow edges)
    let ca_scale = params.dispersion * select(22.0, 30.0, is_base_panel) * (edge * 0.8 + 0.2);
    let ca_offset = (N.xy + cur_dir * pull_t * 0.3) * ca_scale * px_to_uv;
    
    let uv_r = clamp(uv + refr + ca_offset, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_g = clamp(uv + refr, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_b = clamp(uv + refr - ca_offset, vec2<f32>(0.001), vec2<f32>(0.999));
    
    // ════════════════════════════════════════════
    // 5. BACKGROUND SAMPLING & LIQUID CLARITY
    // ════════════════════════════════════════════
    let sharp_r = textureSampleLevel(background_texture, background_sampler, uv_r, 0.0).r;
    let sharp_g = textureSampleLevel(background_texture, background_sampler, uv_g, 0.0).g;
    let sharp_b = textureSampleLevel(background_texture, background_sampler, uv_b, 0.0).b;
    let sharp = vec3<f32>(sharp_r, sharp_g, sharp_b);
    
    var col: vec3<f32>;
    let blur = params.blur_radius;
    if blur <= 0.5 {
        col = sharp;
    } else {
        let blur_r = blurred_sample(uv_r, blur).r;
        let blur_g = blurred_sample(uv_g, blur).g;
        let blur_b = blurred_sample(uv_b, blur).b;
        let blurred = vec3<f32>(blur_r, blur_g, blur_b);
        col = mix(blurred, sharp, pow(edge, 1.4) * 0.45);
    }
    
    // Pure, serene Liquid Glass transmission (liquidglassdesign.com aesthetic)
    if params.tint.a > 0.0 {
        col = mix(col, params.tint.rgb, params.tint.a);
    } else {
        // Pristine clear liquid glass: subtle luminous lift
        col = col * 1.02 + vec3<f32>(0.02, 0.02, 0.03);
    }
    
    // ════════════════════════════════════════════
    // 6. DIRECTIONAL RIM & LIQUID SPECULAR GLEAM
    // ════════════════════════════════════════════
    let L_dir = normalize(vec2<f32>(-0.45, -0.7));
    let N_2d = N.xy;
    
    // Directional meniscus bevel catch (sunlight glint)
    let rim_alignment = max(dot(N_2d, L_dir), 0.0);
    let rim_crest = pow(edge, 1.8) * rim_alignment * 0.50 * params.specular_strength;
    
    // Ambient liquid specular sheen
    let V = vec3<f32>(0.0, 0.0, 1.0);
    let L1 = normalize(vec3<f32>(-0.4, -0.65, 0.75));
    let H1 = normalize(L1 + V);
    let spec1 = pow(max(dot(N, H1), 0.0), 64.0) * 0.40 * params.specular_strength;
    
    // Crisp Apple Liquid Glass rim stroke (1.5px width)
    let rim_stroke = smoothstep(1.8, 0.2, inside);
    let rim_dir = max(dot(N_2d, L_dir), 0.0) * 0.55 + 0.35;
    let border_crest = rim_stroke * rim_dir * 0.75 * params.specular_strength;
    
    // ════════════════════════════════════════════
    // 7. INTERACTIVE CURSOR LIQUID SPOTLIGHT
    // ════════════════════════════════════════════
    var cur_spec = 0.0;
    var cur_rim_glow = 0.0;
    if dist_cur < stretch_radius && params.pointer_pos.x >= 0.0 {
        let cur_t = 1.0 - (dist_cur / stretch_radius);
        let cur_t2 = cur_t * cur_t;
        
        let L_cur = normalize(vec3<f32>(cur_vec, 30.0));
        let H_cur = normalize(L_cur + V);
        cur_spec = pow(max(dot(N, H_cur), 0.0), 96.0) * cur_t2 * 0.40 * params.specular_strength;
        
        let cur_dir_2d = normalize(cur_vec + vec2<f32>(0.001));
        let rim_face_cur = max(dot(N_2d, cur_dir_2d), 0.0);
        cur_rim_glow = pow(edge, 1.6) * rim_face_cur * cur_t * 0.35 * params.specular_strength;
    }
    
    // ════════════════════════════════════════════
    // 8. CONTROLS, FLUID EMISSION & GLYPHS
    // ════════════════════════════════════════════
    let app = render_element_appearance(local_px, half_size, params.padding2.x, params.padding2.y, params.time);
    
    // ════════════════════════════════════════════
    // 9. MASTER COMPOSITE
    // ════════════════════════════════════════════
    var fin = col;
    fin += app.fluid_color * app.fluid_glow;
    fin += vec3<f32>(border_crest + rim_crest);
    fin += vec3<f32>(spec1 + cur_spec + cur_rim_glow);
    fin = mix(fin, app.glyph_color, app.glyph_mask);
    
    return vec4<f32>(fin * mask, mask);
}
