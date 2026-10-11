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

// ── Precision Vector Digit SDFs for High-Resolution Liquid Glass Clock Typography ──
fn draw_digit(p: vec2<f32>, dig: u32, scale: f32) -> f32 {
    let q = p / scale;
    var d = 999.0;
    switch dig {
        case 0u: {
            let outer = rr_sdf(q, vec2<f32>(3.0, 5.0), 2.2);
            d = abs(outer);
        }
        case 1u: {
            let stem = dist_segment(q, vec2<f32>(0.0, -5.0), vec2<f32>(0.0, 5.0));
            let serif = dist_segment(q, vec2<f32>(-2.0, -3.2), vec2<f32>(0.0, -5.0));
            d = min(stem, serif);
        }
        case 2u: {
            let base = dist_segment(q, vec2<f32>(-2.8, 5.0), vec2<f32>(2.8, 5.0));
            let diag = dist_segment(q, vec2<f32>(2.6, -1.8), vec2<f32>(-2.8, 5.0));
            // Semi-circular arch for top of '2' centered at (0.0, -2.4) with radius 2.6
            let arch = abs(length(q - vec2<f32>(0.0, -2.4)) - 2.6);
            let arch_clipped = select(999.0, arch, q.y <= -1.8 || (q.x >= 0.0 && q.y <= -0.5));
            let hook = dist_segment(q, vec2<f32>(-2.6, -2.4), vec2<f32>(-2.6, -1.6));
            d = min(min(base, diag), min(arch_clipped, hook));
        }
        case 3u: {
            let top_arch = abs(length(q - vec2<f32>(0.2, -2.6)) - 2.4);
            let top_clipped = select(999.0, top_arch, q.x >= -0.2 && q.y <= -0.2);
            let top_bar = dist_segment(q, vec2<f32>(-2.6, -5.0), vec2<f32>(0.2, -5.0));
            let bot_arch = abs(length(q - vec2<f32>(0.2, 2.5)) - 2.5);
            let bot_clipped = select(999.0, bot_arch, q.x >= -0.8 && q.y >= 0.0);
            let bot_end = dist_segment(q, vec2<f32>(-2.6, 5.0), vec2<f32>(0.2, 5.0));
            let mid_link = dist_segment(q, vec2<f32>(-0.4, 0.0), vec2<f32>(1.8, 0.0));
            d = min(min(top_bar, top_clipped), min(min(bot_clipped, bot_end), mid_link));
        }
        case 4u: {
            let v_stem = dist_segment(q, vec2<f32>(1.6, -5.0), vec2<f32>(1.6, 5.0));
            let diag = dist_segment(q, vec2<f32>(1.6, -5.0), vec2<f32>(-2.8, 1.5));
            let h_bar = dist_segment(q, vec2<f32>(-2.8, 1.5), vec2<f32>(2.8, 1.5));
            d = min(v_stem, min(diag, h_bar));
        }
        case 5u: {
            let top = dist_segment(q, vec2<f32>(2.8, -5.0), vec2<f32>(-2.6, -5.0));
            let v_upper = dist_segment(q, vec2<f32>(-2.6, -5.0), vec2<f32>(-2.6, -0.5));
            let mid = dist_segment(q, vec2<f32>(-2.6, -0.5), vec2<f32>(0.2, -0.5));
            let bot_arch = abs(length(q - vec2<f32>(0.2, 2.4)) - 2.6);
            let bot_clipped = select(999.0, bot_arch, q.x >= -0.8 && q.y >= -0.5);
            let bot_end = dist_segment(q, vec2<f32>(-2.6, 5.0), vec2<f32>(0.2, 5.0));
            d = min(min(top, v_upper), min(mid, min(bot_clipped, bot_end)));
        }
        case 6u: {
            let bot_loop = abs(length(q - vec2<f32>(0.0, 1.8)) - 2.8);
            let spine = dist_segment(q, vec2<f32>(1.6, -5.0), vec2<f32>(-2.8, 1.8));
            d = min(bot_loop, spine);
        }
        case 7u: {
            let top = dist_segment(q, vec2<f32>(-2.8, -5.0), vec2<f32>(2.8, -5.0));
            let diag = dist_segment(q, vec2<f32>(2.8, -5.0), vec2<f32>(-1.2, 5.0));
            d = min(top, diag);
        }
        case 8u: {
            let top_ring = abs(length(q - vec2<f32>(0.0, -2.4)) - 2.4);
            let bot_ring = abs(length(q - vec2<f32>(0.0, 2.4)) - 2.5);
            d = min(top_ring, bot_ring);
        }
        case 9u: {
            let loop_top = dist_segment(q, vec2<f32>(-2.6, -4.0), vec2<f32>(2.6, -4.0));
            let loop_side = dist_segment(q, vec2<f32>(-2.6, -4.0), vec2<f32>(-2.6, -0.2));
            let loop_bot = dist_segment(q, vec2<f32>(-2.6, -0.2), vec2<f32>(2.6, -0.2));
            let v_upper = dist_segment(q, vec2<f32>(2.6, -4.0), vec2<f32>(2.6, 0.5));
            let spine = dist_segment(q, vec2<f32>(2.6, 0.5), vec2<f32>(-1.8, 4.8));
            d = min(min(loop_top, loop_side), min(loop_bot, min(v_upper, spine)));
        }
        default: {}
    }
    return d * scale;
}

// ── Soft Glowing Pulsing Colon SDF ──
fn draw_colon(p: vec2<f32>, scale: f32) -> f32 {
    let q = p / scale;
    let d_top = length(q - vec2<f32>(0.0, -2.2));
    let d_bot = length(q - vec2<f32>(0.0, 2.2));
    return (min(d_top, d_bot) - 0.75) * scale;
}

// ── Extended Letters for Date & Clock Display ──
fn draw_clock_letter(p: vec2<f32>, ch: u32, scale: f32) -> f32 {
    let q = p / scale;
    var d = 999.0;
    switch ch {
        // 'S'
        case 83u: {
            let top = dist_segment(q, vec2<f32>(2.2, -4.2), vec2<f32>(-1.8, -4.2));
            let side_top = dist_segment(q, vec2<f32>(-1.8, -4.2), vec2<f32>(-1.8, -0.5));
            let mid = dist_segment(q, vec2<f32>(-1.8, -0.5), vec2<f32>(1.8, 0.5));
            let side_bot = dist_segment(q, vec2<f32>(1.8, 0.5), vec2<f32>(1.8, 4.2));
            let bot = dist_segment(q, vec2<f32>(1.8, 4.2), vec2<f32>(-2.2, 4.2));
            d = min(min(top, side_top), min(mid, min(side_bot, bot)));
        }
        // 'U'
        case 85u: {
            let left = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(-2.2, 1.5));
            let right = dist_segment(q, vec2<f32>(2.2, -4.2), vec2<f32>(2.2, 1.5));
            let bot = dist_segment(q, vec2<f32>(-2.2, 4.2), vec2<f32>(2.2, 4.2));
            d = min(min(left, right), bot);
        }
        // 'N'
        case 78u: {
            d = min(dist_segment(q, vec2<f32>(-2.4, -4.2), vec2<f32>(-2.4, 4.2)),
                min(dist_segment(q, vec2<f32>(-2.4, -4.2), vec2<f32>(2.4, 4.2)),
                    dist_segment(q, vec2<f32>(2.4, -4.2), vec2<f32>(2.4, 4.2))));
        }
        // 'M'
        case 77u: {
            d = min(dist_segment(q, vec2<f32>(-2.8, 4.2), vec2<f32>(-2.8, -4.2)),
                min(dist_segment(q, vec2<f32>(-2.8, -4.2), vec2<f32>(0.0, 0.5)),
                min(dist_segment(q, vec2<f32>(0.0, 0.5), vec2<f32>(2.8, -4.2)),
                    dist_segment(q, vec2<f32>(2.8, -4.2), vec2<f32>(2.8, 4.2)))));
        }
        // 'T'
        case 84u: {
            d = min(dist_segment(q, vec2<f32>(-2.5, -4.2), vec2<f32>(2.5, -4.2)),
                    dist_segment(q, vec2<f32>(0.0, -4.2), vec2<f32>(0.0, 4.2)));
        }
        // 'W'
        case 87u: {
            d = min(dist_segment(q, vec2<f32>(-2.8, -4.2), vec2<f32>(-1.5, 4.2)),
                min(dist_segment(q, vec2<f32>(-1.5, 4.2), vec2<f32>(0.0, -0.5)),
                min(dist_segment(q, vec2<f32>(0.0, -0.5), vec2<f32>(1.5, 4.2)),
                    dist_segment(q, vec2<f32>(1.5, 4.2), vec2<f32>(2.8, -4.2)))));
        }
        // 'F'
        case 70u: {
            d = min(dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(-2.2, 4.2)),
                min(dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(2.2, -4.2)),
                    dist_segment(q, vec2<f32>(-2.2, 0.0), vec2<f32>(1.5, 0.0))));
        }
        // 'R'
        case 82u: {
            let stem = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(-2.2, 4.2));
            let top = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(1.5, -4.2));
            let mid = dist_segment(q, vec2<f32>(-2.2, 0.0), vec2<f32>(1.5, 0.0));
            let side = dist_segment(q, vec2<f32>(1.5, -4.2), vec2<f32>(1.5, 0.0));
            let leg = dist_segment(q, vec2<f32>(-0.5, 0.0), vec2<f32>(2.2, 4.2));
            d = min(min(stem, top), min(mid, min(side, leg)));
        }
        // 'A'
        case 65u: {
            d = min(dist_segment(q, vec2<f32>(-2.4, 4.2), vec2<f32>(0.0, -4.2)),
                min(dist_segment(q, vec2<f32>(0.0, -4.2), vec2<f32>(2.4, 4.2)),
                    dist_segment(q, vec2<f32>(-1.3, 1.0), vec2<f32>(1.3, 1.0))));
        }
        // 'H'
        case 72u: {
            d = min(dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(-2.2, 4.2)),
                min(dist_segment(q, vec2<f32>(2.2, -4.2), vec2<f32>(2.2, 4.2)),
                    dist_segment(q, vec2<f32>(-2.2, 0.0), vec2<f32>(2.2, 0.0))));
        }
        // 'O'
        case 79u: {
            d = abs(length(q) - 3.2);
        }
        // 'C'
        case 67u: {
            let ang = atan2(q.y, q.x);
            if abs(ang) < 0.85 {
                let end_top = vec2<f32>(3.2 * cos(0.85), -3.2 * sin(0.85));
                let end_bot = vec2<f32>(3.2 * cos(0.85),  3.2 * sin(0.85));
                d = min(length(q - end_top), length(q - end_bot));
            } else {
                d = abs(length(q) - 3.2);
            }
        }
        // 'E'
        case 69u: {
            d = min(dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(-2.2, 4.2)),
                min(dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(2.0, -4.2)),
                min(dist_segment(q, vec2<f32>(-2.2, 0.0),  vec2<f32>(1.4, 0.0)),
                    dist_segment(q, vec2<f32>(-2.2, 4.2),  vec2<f32>(2.0, 4.2)))));
        }
        // 'P'
        case 80u: {
            let stem = dist_segment(q, vec2<f32>(-2.2, 4.2), vec2<f32>(-2.2, -4.2));
            let loop_top = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(1.5, -4.2));
            let loop_bot = dist_segment(q, vec2<f32>(-2.2, -0.2), vec2<f32>(1.5, -0.2));
            let loop_side = dist_segment(q, vec2<f32>(1.5, -4.2), vec2<f32>(1.5, -0.2));
            d = min(stem, min(loop_top, min(loop_bot, loop_side)));
        }
        default: {}
    }
    return d * scale;
}

// ── Serene Liquid Glass Clock Appearance ──
fn render_element_appearance(local_p: vec2<f32>, half: vec2<f32>, el_type: f32, progress: f32, time: f32, tint: vec4<f32>) -> ElementAppearance {
    var app: ElementAppearance;
    app.glyph_mask = 0.0;
    app.glyph_color = vec3<f32>(1.0, 1.0, 1.0);
    app.fluid_color = vec3<f32>(1.0, 1.0, 1.0);
    app.fluid_glow = 0.0;
    
    if el_type <= 0.0 {
        return app;
    }

    // ─────────────────────────────────────────────────────────────
    // 20. Left Element: Swiss Minimalist Precision Analog Dial (120px)
    // ─────────────────────────────────────────────────────────────
    if el_type >= 19.9 && el_type <= 20.1 {
        let r_p = length(local_p);
        let ang = atan2(local_p.y, local_p.x); // [-pi, pi]
        
        let hr = tint.x;
        let mn = tint.y;
        let sec = tint.z;
        
        // 1. Subtle crystalline dark obsidian tint dial backing (r <= 54px) for 100% contrast against bright backgrounds
        let dial_disk = smoothstep(54.0, 52.0, r_p);
        let dial_backing_mask = dial_disk * 0.40;
        
        // 12 Minimalist Precision Hour Ticks (Scale tuned for 120px diameter, r_outer = 50px)
        let mod_ang = (ang + 3.14159265 / 12.0) % (3.14159265 / 6.0) - (3.14159265 / 12.0);
        let tick_dist = abs(mod_ang) * r_p;
        // Cardinal ticks (12, 3, 6, 9) are solid Swiss batons
        let mod_card = (ang + 3.14159265 / 4.0) % (3.14159265 / 2.0) - (3.14159265 / 4.0);
        let is_cardinal = (abs(mod_card) * r_p < 3.2);
        let tick_inner = select(44.0, 36.0, is_cardinal);
        let tick_rad = (r_p >= tick_inner && r_p <= 49.5);
        var tick_mask = 0.0;
        if tick_rad {
            let tick_w = select(0.9, 1.6, is_cardinal);
            tick_mask = 1.0 - smoothstep(tick_w, tick_w + 0.50, tick_dist);
        }
        
        // Analog Hands angles (clockwise from 12 o'clock = -pi/2)
        let hour_angle = (hr % 12.0 + mn / 60.0 + sec / 3600.0) * (3.14159265 / 6.0) - 1.5707963;
        let min_angle = (mn + sec / 60.0) * (3.14159265 / 30.0) - 1.5707963;
        let sec_angle = sec * (3.14159265 / 30.0) - 1.5707963;
        
        let p_hour = vec2<f32>(cos(hour_angle), sin(hour_angle));
        let p_min = vec2<f32>(cos(min_angle), sin(min_angle));
        let p_sec = vec2<f32>(cos(sec_angle), sin(sec_angle));
        
        // Hour hand: sculpted baton (4.2px width), length 29px, counter-weight 6px
        let d_hour = dist_segment(local_p, -p_hour * 6.0, p_hour * 29.0) - 2.1;
        let mask_hour = 1.0 - smoothstep(-0.4, 0.4, d_hour);
        
        // Minute hand: sleek baton (3.0px width), length 43px, counter-weight 7px
        let d_min = dist_segment(local_p, -p_min * 7.0, p_min * 43.0) - 1.5;
        let mask_min = 1.0 - smoothstep(-0.4, 0.4, d_min);
        
        // Center Pin: double-ring jeweled pivot
        let d_pin = length(local_p) - 4.5;
        let mask_pin = 1.0 - smoothstep(-0.35, 0.35, d_pin);
        let d_pin_core = length(local_p) - 2.2;
        let mask_pin_core = 1.0 - smoothstep(-0.35, 0.35, d_pin_core);
        
        // Second hand: signature radiant lemon-gold #DCF842, length 50px with counter-weight
        let d_sec = dist_segment(local_p, -p_sec * 12.0, p_sec * 50.0) - 0.85;
        let mask_sec = 1.0 - smoothstep(-0.35, 0.35, d_sec);
        
        let gold = vec3<f32>(0.86, 0.97, 0.26);
        let white = vec3<f32>(0.98, 0.99, 1.0);
        let dark_backing = vec3<f32>(0.02, 0.03, 0.05);
        
        // Outer aesthetic perimeter glint ring at r = 52.5
        let outer_ring = abs(r_p - 52.5) - 0.75;
        let mask_ring = (1.0 - smoothstep(-0.35, 0.35, outer_ring)) * 0.65;
        
        let white_mask = max(max(tick_mask, mask_hour), max(mask_min, mask_pin));
        
        if mask_sec > 0.05 {
            app.glyph_mask = mask_sec;
            app.glyph_color = gold;
            app.fluid_color = gold;
            app.fluid_glow = exp(-max(d_sec, 0.0) * 0.8) * 0.45;
        } else if mask_pin_core > 0.05 {
            app.glyph_mask = mask_pin_core;
            app.glyph_color = gold;
        } else if white_mask > 0.05 || mask_ring > 0.05 {
            app.glyph_mask = max(white_mask, mask_ring);
            app.glyph_color = white;
        } else if dial_backing_mask > 0.05 {
            app.glyph_mask = dial_backing_mask;
            app.glyph_color = dark_backing;
        }
        return app;
    }

    // ─────────────────────────────────────────────────────────────
    // 21. Center/Right Element: Digital Liquid Glass Clock + Battery Capsule
    // ─────────────────────────────────────────────────────────────
    if el_type >= 20.9 && el_type <= 21.1 {
        let is_12h = (tint.w > 0.5);
        var hr = tint.x;
        if is_12h {
            if hr == 0.0 {
                hr = 12.0;
            } else if hr > 12.0 {
                hr = hr - 12.0;
            }
        }
        let mn = tint.y;
        let sec = tint.z;
        
        let h_tens = u32(hr / 10.0);
        let h_ones = u32(hr % 10.0);
        let m_tens = u32(mn / 10.0);
        let m_ones = u32(mn % 10.0);
        
        // Digital Clock Row: Centered at center_x = -36.0, y = -10.0
        let center_x = -36.0;
        let clock_y = -10.0;
        let num_scale = 3.0;
        let sp = 20.5;
        
        let d_h0 = draw_digit(local_p - vec2<f32>(center_x - 2.1 * sp, clock_y), h_tens, num_scale);
        let d_h1 = draw_digit(local_p - vec2<f32>(center_x - 1.1 * sp, clock_y), h_ones, num_scale);
        let d_colon = draw_colon(local_p - vec2<f32>(center_x, clock_y), num_scale);
        let d_m0 = draw_digit(local_p - vec2<f32>(center_x + 1.1 * sp, clock_y), m_tens, num_scale);
        let d_m1 = draw_digit(local_p - vec2<f32>(center_x + 2.1 * sp, clock_y), m_ones, num_scale);
        
        // Pulse colon gently with fractional seconds
        let colon_pulse = 0.55 + 0.45 * sin(sec * 3.14159265);
        
        let d_digits = min(min(d_h0, d_h1), min(d_m0, d_m1)) - 0.85;
        let mask_digits = 1.0 - smoothstep(-0.4, 0.4, d_digits);
        let mask_colon = (1.0 - smoothstep(-0.35, 0.35, d_colon)) * colon_pulse;
        
        // Date / Day Row: Elegant sub-row (y = 22.0)
        // Decoded from progress: wday * 10000 + mday * 100 + (bat / 10)
        let wday_code = u32(progress / 10000.0);
        let rem_date = progress % 10000.0;
        let mday_code = u32(rem_date / 100.0);
        let bat_val = (rem_date % 100.0); // 0.0 - 100.0
        
        let d_tens = mday_code / 10u;
        let d_ones = mday_code % 10u;
        
        let date_y = 22.0;
        let date_scale = 1.15;
        let dsp = 9.8;
        let date_cx = center_x;
        
        // Weekday 3 letters: e.g. SUN, MON, TUE, WED, THU, FRI, SAT
        var w0 = 83u; var w1 = 85u; var w2 = 78u; // SUN
        if wday_code == 1u { w0 = 77u; w1 = 79u; w2 = 78u; } // MON
        else if wday_code == 2u { w0 = 84u; w1 = 85u; w2 = 69u; } // TUE
        else if wday_code == 3u { w0 = 87u; w1 = 69u; w2 = 68u; } // WED
        else if wday_code == 4u { w0 = 84u; w1 = 72u; w2 = 85u; } // THU
        else if wday_code == 5u { w0 = 70u; w1 = 82u; w2 = 73u; } // FRI
        else if wday_code == 6u { w0 = 83u; w1 = 65u; w2 = 84u; } // SAT
        
        let dw0 = draw_clock_letter(local_p - vec2<f32>(date_cx - 2.8 * dsp, date_y), w0, date_scale);
        let dw1 = draw_clock_letter(local_p - vec2<f32>(date_cx - 1.8 * dsp, date_y), w1, date_scale);
        let dw2 = draw_clock_letter(local_p - vec2<f32>(date_cx - 0.8 * dsp, date_y), w2, date_scale);
        
        // Date day number
        let dd0 = draw_digit(local_p - vec2<f32>(date_cx + 1.0 * dsp, date_y), d_tens, date_scale);
        let dd1 = draw_digit(local_p - vec2<f32>(date_cx + 2.0 * dsp, date_y), d_ones, date_scale);
        
        let d_date = min(min(dw0, min(dw1, dw2)), min(dd0, dd1)) - 0.52;
        let mask_date = (1.0 - smoothstep(-0.35, 0.35, d_date)) * 0.82;

        // ── Sleek Compact Liquid Glass Battery Pill (Apple-style horizontal battery icon) ──
        let bat_cx = 124.0;
        let bat_cy = 0.0;
        let p_bat = local_p - vec2<f32>(bat_cx, bat_cy);
        
        // Compact Battery Housing: 40px width x 20px height, corner radius 5px
        let bat_half = vec2<f32>(20.0, 10.0);
        let d_shell = rr_sdf(p_bat, bat_half, 5.0);
        // Outer housing stroke (1.4px)
        let mask_shell = smoothstep(0.4, -0.4, abs(d_shell) - 0.7);
        
        // Battery positive terminal bump on the right side (+x = 22.5px, h = 8px)
        let d_bump = rr_sdf(p_bat - vec2<f32>(22.2, 0.0), vec2<f32>(2.2, 4.0), 2.0);
        let mask_bump = smoothstep(0.4, -0.4, d_bump);
        let mask_housing = max(mask_shell, mask_bump) * 0.75;
        
        // Dynamic interior battery fill level according to bat_val (0 - 100%)
        let fill_max_w = 34.0;
        let fill_w = (bat_val / 100.0) * fill_max_w;
        let fill_x = -17.0 + fill_w * 0.5;
        let d_fill = rr_sdf(p_bat - vec2<f32>(fill_x, 0.0), vec2<f32>(max(fill_w * 0.5, 1.0), 7.0), 3.0);
        let mask_fill = smoothstep(0.4, -0.4, d_fill) * 0.85;
        
        // Battery Percentage Number below the icon (y = 22.0) matching the date row baseline
        let bat_u = u32(clamp(bat_val, 0.0, 100.0));
        let b_tens = bat_u / 10u;
        let b_ones = bat_u % 10u;
        let b_scale = 1.05;
        let bsp = 7.2;
        let db0 = draw_digit(local_p - vec2<f32>(bat_cx - 0.8 * bsp, 22.0), b_tens, b_scale);
        let db1 = draw_digit(local_p - vec2<f32>(bat_cx + 0.4 * bsp, 22.0), b_ones, b_scale);
        let d_bnum = min(db0, db1) - 0.45;
        let mask_bnum = (1.0 - smoothstep(-0.35, 0.35, d_bnum)) * 0.85;
        
        let white_elems = max(max(mask_digits, mask_colon), max(mask_date, max(mask_housing, mask_bnum)));
        let gold = vec3<f32>(0.86, 0.97, 0.26);
        let white = vec3<f32>(0.98, 0.99, 1.0);
        
        // Proper layering: battery gold fill + housing + crisp white typography
        if mask_fill > 0.05 {
            app.glyph_mask = max(mask_fill, white_elems);
            // Blend fill with outer shell/numbers if overlapping
            app.glyph_color = mix(gold, white, clamp((white_elems - mask_fill) * 2.0, 0.0, 1.0));
            app.fluid_color = gold;
            app.fluid_glow = exp(-max(d_fill, 0.0) * 0.6) * 0.30;
        } else {
            app.glyph_mask = white_elems;
            app.glyph_color = white;
        }
        return app;
    }

    // ─────────────────────────────────────────────────────────────
    // 22. Nested Seconds Liquid Glass Pill
    // ─────────────────────────────────────────────────────────────
    if el_type >= 21.9 && el_type <= 22.1 {
        let sec = tint.z;
        let sec_u = u32(sec);
        let s_tens = sec_u / 10u;
        let s_ones = sec_u % 10u;
        
        let scale = 1.25;
        let sp = 8.0;
        let d0 = draw_digit(local_p - vec2<f32>(-0.6 * sp, 0.0), s_tens, scale);
        let d1 = draw_digit(local_p - vec2<f32>( 0.6 * sp, 0.0), s_ones, scale);
        
        let d_sec = min(d0, d1) - 0.50;
        let mask = 1.0 - smoothstep(-0.35, 0.35, d_sec);
        
        // Signature radiant liquid gold #DCF842
        let gold = vec3<f32>(0.86, 0.97, 0.26);
        app.glyph_mask = mask * 0.98;
        app.glyph_color = gold;
        
        let bloom = exp(-max(d_sec, 0.0) * 0.70) * 0.35;
        app.fluid_color = gold;
        app.fluid_glow = bloom;
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
    // Note: On Wayland transparent layer shells, emitting dark RGB with non-zero alpha
    // causes compositor dark fringes against wallpapers. Zero outside SDF.
    if sdf > 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
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
    
    // Reactive subtle fluid meniscus pull towards pointer
    let cur_vec = cur_local - local_px;
    let dist_cur = length(cur_vec);
    let cur_dir = cur_vec / max(dist_cur, 0.001);
    let stretch_radius = select(100.0, 160.0, is_base_panel);
    let pull_t = clamp(1.0 - (dist_cur / stretch_radius), 0.0, 1.0) * cur_active;
    let fluid_tilt = cur_dir * (pull_t * pull_t) * 0.04;
    
    let N = normalize(vec3<f32>(-(h_grad * params.curvature * 2.2 + fluid_tilt), 1.0));
    
    // ════════════════════════════════════════════
    // 4. TRUE OPTICAL REFRACTION & CHROMATIC DISPERSION (Snell's Law)
    // ════════════════════════════════════════════
    let px_to_uv = vec2<f32>(1.0, 1.0) / params.surface_size;
    let ior = params.ior;
    let refr_pow = 1.0 - 1.0 / ior;
    
    // Pure optical ray bending through liquid glass lens
    let refr_scale = select(10.0, 14.0, is_base_panel) * params.refraction_strength;
    let refr_ray = N.xy * refr_pow * refr_scale;
    let refr = refr_ray * px_to_uv;
    
    // Chromatic dispersion (subtle prismatic crystal fringe)
    let ca_scale = params.dispersion * select(10.0, 14.0, is_base_panel) * (edge * 0.8 + 0.2);
    let ca_offset = N.xy * ca_scale * px_to_uv;
    
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
    
    // Pure, serene Liquid Glass transmission (luxury Apple frosted crystalline glass)
    if params.padding2.x >= 19.5 {
        // Clock elements: luxury translucent crystalline plate (15% soft white frost + 6% dark contrast ground)
        // This guarantees pristine visibility and tactile depth on any wallpaper, whether bright or dark.
        let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
        let frost = vec3<f32>(0.12, 0.14, 0.18);
        col = mix(col, frost, 0.32);
        col = col + vec3<f32>(0.04, 0.05, 0.07);
    } else if params.tint.a > 0.0 {
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
    let app = render_element_appearance(local_px, half_size, params.padding2.x, params.padding2.y, params.time, params.tint);
    
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
