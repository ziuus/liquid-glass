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

// ── Authentic Physical Planar Glass Lens with Perimeter Meniscus ──
fn droplet_height(p: vec2<f32>, b: vec2<f32>, r: f32, z_radius: f32, cur_px: vec2<f32>, cur_vel: vec2<f32>, time: f32, cur_active: f32) -> f32 {
    let d = -rr_sdf(p, b, r);
    if d <= 0.0 {
        return 0.0;
    }

    let bevel_w = clamp(z_radius * 1.5, 6.0, 16.0);
    if d >= bevel_w {
        // Planar interior face: constant height, zero tilt!
        var h = z_radius;
        if cur_active > 0.5 {
            let to_cur = cur_px - p;
            let dist = length(to_cur);
            let pull = exp(-(dist * dist) / (2.0 * 50.0 * 50.0)) * (z_radius * 0.15);
            h += pull;
        }
        return h;
    }

    // Cylindrical quadrant meniscus bevel at the perimeter
    let t = clamp(d / bevel_w, 0.0, 1.0);
    let h_bevel = z_radius * sqrt(clamp(1.0 - (1.0 - t) * (1.0 - t), 0.0, 1.0));
    return h_bevel;
}

// ── Apple CoreAnimation-grade 16-tap Poisson Blur (Single-Fetch Bilinear Hardware Filtering) ──
fn sample_frost_blur(uv: vec2<f32>, radius: f32) -> vec3<f32> {
    if radius <= 0.5 {
        return textureSampleLevel(background_texture, background_sampler, uv, 0.0).rgb;
    }
    let rad_uv = radius / params.surface_size;
    let golden_angle = 2.39996323;

    var col = textureSampleLevel(background_texture, background_sampler, uv, 0.0).rgb * 0.18;
    var total_w = 0.18;

    for (var i = 0.0; i < 16.0; i += 1.0) {
        let t = (i + 0.5) / 16.0;
        let r = sqrt(t);
        let theta = i * golden_angle;
        let offset = vec2<f32>(r * cos(theta), r * sin(theta)) * rad_uv;
        let sample_uv = clamp(uv + offset, vec2<f32>(0.001), vec2<f32>(0.999));
        let s = textureSampleLevel(background_texture, background_sampler, sample_uv, 0.0).rgb;
        let w = exp(-r * r * 2.2);
        col += s * w;
        total_w += w;
    }

    return col / total_w;
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

// ── Precision Percentage Symbol SDF for Battery Percentage ──
fn draw_percent(p: vec2<f32>, scale: f32) -> f32 {
    let q = p / scale;
    let slash = dist_segment(q, vec2<f32>(-2.0, 3.2), vec2<f32>(2.0, -3.2));
    let dot1 = length(q - vec2<f32>(-1.4, -1.8)) - 0.85;
    let dot2 = length(q - vec2<f32>(1.4, 1.8)) - 0.85;
    return min(slash, min(dot1, dot2)) * scale;
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
            let left = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(-2.2, 2.2));
            let right = dist_segment(q, vec2<f32>(2.2, -4.2), vec2<f32>(2.2, 2.2));
            let bot = dist_segment(q, vec2<f32>(-2.2, 4.2), vec2<f32>(2.2, 4.2));
            let c_l = dist_segment(q, vec2<f32>(-2.2, 2.2), vec2<f32>(-2.2, 4.2));
            let c_r = dist_segment(q, vec2<f32>(2.2, 2.2), vec2<f32>(2.2, 4.2));
            d = min(min(left, right), min(bot, min(c_l, c_r)));
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
        // 'J'
        case 74u: {
            let stem = dist_segment(q, vec2<f32>(1.8, -4.2), vec2<f32>(1.8, 2.2));
            let curve = dist_segment(q, vec2<f32>(1.8, 2.2), vec2<f32>(-1.8, 4.2));
            let hook = dist_segment(q, vec2<f32>(-1.8, 4.2), vec2<f32>(-1.8, 2.0));
            d = min(stem, min(curve, hook));
        }
        // 'B'
        case 66u: {
            let stem = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(-2.2, 4.2));
            let top = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(1.4, -4.2));
            let mid = dist_segment(q, vec2<f32>(-2.2, 0.0), vec2<f32>(1.4, 0.0));
            let bot = dist_segment(q, vec2<f32>(-2.2, 4.2), vec2<f32>(1.4, 4.2));
            let s1 = dist_segment(q, vec2<f32>(1.4, -4.2), vec2<f32>(1.4, 0.0));
            let s2 = dist_segment(q, vec2<f32>(1.4, 0.0), vec2<f32>(1.4, 4.2));
            d = min(min(stem, top), min(min(mid, bot), min(s1, s2)));
        }
        // 'Y'
        case 89u: {
            let l = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(0.0, -0.5));
            let r = dist_segment(q, vec2<f32>(2.2, -4.2), vec2<f32>(0.0, -0.5));
            let stem = dist_segment(q, vec2<f32>(0.0, -0.5), vec2<f32>(0.0, 4.2));
            d = min(min(l, r), stem);
        }
        // 'L'
        case 76u: {
            let stem = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(-2.2, 4.2));
            let bot = dist_segment(q, vec2<f32>(-2.2, 4.2), vec2<f32>(2.0, 4.2));
            d = min(stem, bot);
        }
        // 'G'
        case 71u: {
            let top = dist_segment(q, vec2<f32>(2.0, -4.2), vec2<f32>(-2.2, -4.2));
            let stem = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(-2.2, 4.2));
            let bot = dist_segment(q, vec2<f32>(-2.2, 4.2), vec2<f32>(2.0, 4.2));
            let up = dist_segment(q, vec2<f32>(2.0, 4.2), vec2<f32>(2.0, 0.0));
            let bar = dist_segment(q, vec2<f32>(0.0, 0.0), vec2<f32>(2.0, 0.0));
            d = min(min(top, stem), min(bot, min(up, bar)));
        }
        // 'V'
        case 86u: {
            let l = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(0.0, 4.2));
            let r = dist_segment(q, vec2<f32>(2.2, -4.2), vec2<f32>(0.0, 4.2));
            d = min(l, r);
        }
        // 'D'
        case 68u: {
            let stem = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(-2.2, 4.2));
            let top = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(0.8, -4.2));
            let bot = dist_segment(q, vec2<f32>(-2.2, 4.2), vec2<f32>(0.8, 4.2));
            let diag = dist_segment(q, vec2<f32>(0.8, -4.2), vec2<f32>(2.2, 0.0));
            let diag2 = dist_segment(q, vec2<f32>(2.2, 0.0), vec2<f32>(0.8, 4.2));
            d = min(stem, min(top, min(bot, min(diag, diag2))));
        }
        // 'I'
        case 73u: {
            d = dist_segment(q, vec2<f32>(0.0, -4.2), vec2<f32>(0.0, 4.2));
        }
        // '('
        case 40u: {
            let top = dist_segment(q, vec2<f32>(1.0, -4.2), vec2<f32>(-1.0, 0.0));
            let bot = dist_segment(q, vec2<f32>(-1.0, 0.0), vec2<f32>(1.0, 4.2));
            d = min(top, bot);
        }
        // ')'
        case 41u: {
            let top = dist_segment(q, vec2<f32>(-1.0, -4.2), vec2<f32>(1.0, 0.0));
            let bot = dist_segment(q, vec2<f32>(1.0, 0.0), vec2<f32>(-1.0, 4.2));
            d = min(top, bot);
        }
        // 'K'
        case 75u: {
            let stem = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(-2.2, 4.2));
            let top = dist_segment(q, vec2<f32>(-2.2, 0.0), vec2<f32>(2.0, -4.2));
            let bot = dist_segment(q, vec2<f32>(-1.0, -1.2), vec2<f32>(2.0, 4.2));
            d = min(stem, min(top, bot));
        }
        // 'X'
        case 88u: {
            let d1 = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(2.2, 4.2));
            let d2 = dist_segment(q, vec2<f32>(2.2, -4.2), vec2<f32>(-2.2, 4.2));
            d = min(d1, d2);
        }
        // 'Z'
        case 90u: {
            let top = dist_segment(q, vec2<f32>(-2.2, -4.2), vec2<f32>(2.2, -4.2));
            let diag = dist_segment(q, vec2<f32>(2.2, -4.2), vec2<f32>(-2.2, 4.2));
            let bot = dist_segment(q, vec2<f32>(-2.2, 4.2), vec2<f32>(2.2, 4.2));
            d = min(min(top, diag), bot);
        }
        // '&'
        case 38u: {
            let l_top = abs(length(q - vec2<f32>(0.0, -1.8)) - 1.8);
            let l_bot = abs(length(q - vec2<f32>(0.2, 1.8)) - 2.2);
            let tail = dist_segment(q, vec2<f32>(-1.4, 0.0), vec2<f32>(2.4, 4.0));
            d = min(min(l_top, l_bot), tail);
        }
        default: {}
    }
    return d * scale;
}

// ── Precision Checkmark SDF ──
fn draw_checkmark(p: vec2<f32>, scale: f32) -> f32 {
    let q = p / scale;
    let leg1 = dist_segment(q, vec2<f32>(-3.2, 0.2), vec2<f32>(-1.0, 3.2));
    let leg2 = dist_segment(q, vec2<f32>(-1.0, 3.2), vec2<f32>(3.6, -3.2));
    return (min(leg1, leg2) - 0.75) * scale;
}

// ── Evaluator for Todo Row Typography ──
fn eval_todo_row_text(p: vec2<f32>, row_idx: u32) -> f32 {
    let row_y = -105.0 + f32(row_idx) * 44.0;
    let tx_start = -112.0;
    let tx_scale = 1.05;
    let sp = 8.2;
    var d = 999.0;
    if row_idx == 0u {
        // LEETCODE
        let l0 = draw_clock_letter(p - vec2<f32>(tx_start + 0.0 * sp, row_y), 76u, tx_scale); // L
        let l1 = draw_clock_letter(p - vec2<f32>(tx_start + 1.0 * sp, row_y), 69u, tx_scale); // E
        let l2 = draw_clock_letter(p - vec2<f32>(tx_start + 2.0 * sp, row_y), 69u, tx_scale); // E
        let l3 = draw_clock_letter(p - vec2<f32>(tx_start + 3.0 * sp, row_y), 84u, tx_scale); // T
        let l4 = draw_clock_letter(p - vec2<f32>(tx_start + 4.0 * sp, row_y), 67u, tx_scale); // C
        let l5 = draw_clock_letter(p - vec2<f32>(tx_start + 5.0 * sp, row_y), 79u, tx_scale); // O
        let l6 = draw_clock_letter(p - vec2<f32>(tx_start + 6.0 * sp, row_y), 68u, tx_scale); // D
        let l7 = draw_clock_letter(p - vec2<f32>(tx_start + 7.0 * sp, row_y), 69u, tx_scale); // E
        d = min(min(min(l0, l1), min(l2, l3)), min(min(l4, l5), min(l6, l7)));
    } else if row_idx == 1u {
        // LEARN
        let l0 = draw_clock_letter(p - vec2<f32>(tx_start + 0.0 * sp, row_y), 76u, tx_scale); // L
        let l1 = draw_clock_letter(p - vec2<f32>(tx_start + 1.0 * sp, row_y), 69u, tx_scale); // E
        let l2 = draw_clock_letter(p - vec2<f32>(tx_start + 2.0 * sp, row_y), 65u, tx_scale); // A
        let l3 = draw_clock_letter(p - vec2<f32>(tx_start + 3.0 * sp, row_y), 82u, tx_scale); // R
        let l4 = draw_clock_letter(p - vec2<f32>(tx_start + 4.0 * sp, row_y), 78u, tx_scale); // N
        d = min(min(min(l0, l1), min(l2, l3)), l4);
    } else if row_idx == 2u {
        // CERT
        let l0 = draw_clock_letter(p - vec2<f32>(tx_start + 0.0 * sp, row_y), 67u, tx_scale); // C
        let l1 = draw_clock_letter(p - vec2<f32>(tx_start + 1.0 * sp, row_y), 69u, tx_scale); // E
        let l2 = draw_clock_letter(p - vec2<f32>(tx_start + 2.0 * sp, row_y), 82u, tx_scale); // R
        let l3 = draw_clock_letter(p - vec2<f32>(tx_start + 3.0 * sp, row_y), 84u, tx_scale); // T
        d = min(min(l0, l1), min(l2, l3));
    } else if row_idx == 3u {
        // CAREER
        let l0 = draw_clock_letter(p - vec2<f32>(tx_start + 0.0 * sp, row_y), 67u, tx_scale); // C
        let l1 = draw_clock_letter(p - vec2<f32>(tx_start + 1.0 * sp, row_y), 65u, tx_scale); // A
        let l2 = draw_clock_letter(p - vec2<f32>(tx_start + 2.0 * sp, row_y), 82u, tx_scale); // R
        let l3 = draw_clock_letter(p - vec2<f32>(tx_start + 3.0 * sp, row_y), 69u, tx_scale); // E
        let l4 = draw_clock_letter(p - vec2<f32>(tx_start + 4.0 * sp, row_y), 69u, tx_scale); // E
        let l5 = draw_clock_letter(p - vec2<f32>(tx_start + 5.0 * sp, row_y), 82u, tx_scale); // R
        d = min(min(min(l0, l1), min(l2, l3)), min(l4, l5));
    } else if row_idx == 4u {
        // HACKERRANK
        let l0 = draw_clock_letter(p - vec2<f32>(tx_start + 0.0 * sp, row_y), 72u, tx_scale); // H
        let l1 = draw_clock_letter(p - vec2<f32>(tx_start + 0.9 * sp, row_y), 65u, tx_scale); // A
        let l2 = draw_clock_letter(p - vec2<f32>(tx_start + 1.8 * sp, row_y), 67u, tx_scale); // C
        let l3 = draw_clock_letter(p - vec2<f32>(tx_start + 2.7 * sp, row_y), 75u, tx_scale); // K
        let l4 = draw_clock_letter(p - vec2<f32>(tx_start + 3.6 * sp, row_y), 69u, tx_scale); // E
        let l5 = draw_clock_letter(p - vec2<f32>(tx_start + 4.5 * sp, row_y), 82u, tx_scale); // R
        let l6 = draw_clock_letter(p - vec2<f32>(tx_start + 5.4 * sp, row_y), 82u, tx_scale); // R
        let l7 = draw_clock_letter(p - vec2<f32>(tx_start + 6.3 * sp, row_y), 65u, tx_scale); // A
        let l8 = draw_clock_letter(p - vec2<f32>(tx_start + 7.2 * sp, row_y), 78u, tx_scale); // N
        let l9 = draw_clock_letter(p - vec2<f32>(tx_start + 8.1 * sp, row_y), 75u, tx_scale); // K
        d = min(min(min(min(l0, l1), min(l2, l3)), min(l4, l5)), min(min(l6, l7), min(l8, l9)));
    } else if row_idx == 5u {
        // AI & ML
        let l0 = draw_clock_letter(p - vec2<f32>(tx_start + 0.0 * sp, row_y), 65u, tx_scale); // A
        let l1 = draw_clock_letter(p - vec2<f32>(tx_start + 1.1 * sp, row_y), 73u, tx_scale); // I
        let l2 = draw_clock_letter(p - vec2<f32>(tx_start + 2.5 * sp, row_y), 38u, tx_scale); // &
        let l3 = draw_clock_letter(p - vec2<f32>(tx_start + 4.0 * sp, row_y), 77u, tx_scale); // M
        let l4 = draw_clock_letter(p - vec2<f32>(tx_start + 5.2 * sp, row_y), 76u, tx_scale); // L
        d = min(min(min(l0, l1), l2), min(l3, l4));
    }
    return d;
}

// ── Serene Liquid Glass Clock Appearance ──
fn render_element_appearance(local_p: vec2<f32>, half: vec2<f32>, el_type: f32, progress: f32, time: f32, tint: vec4<f32>, is_light: f32) -> ElementAppearance {
    var app: ElementAppearance;
    app.glyph_mask = 0.0;
    app.glyph_color = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.11, 0.14, 0.18), is_light);
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
        let gold = mix(vec3<f32>(0.86, 0.97, 0.26), vec3<f32>(0.14, 0.52, 0.26), is_light);
        let white = mix(vec3<f32>(0.98, 0.99, 1.0), vec3<f32>(0.11, 0.14, 0.18), is_light);

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
    // 25. Unified Luxury Minimalist Liquid Glass Widget (320px x 218px)
    // Pure Glassmorphism:
    // Top: Frosted Day Pill (`• SUNDAY`) + Frosted Battery Pill (`⚡ 60%`)
    // Center: Hero Digital Clock (`12:52` PM) with high-contrast adaptive typography
    // Bottom: Frosted Date Pill (`📅 NOV 01`) + Precision Liquid Battery Fill Track
    // Adapts to wallpaper accent color or pure crystalline glass ("otherwise just no color")
    // ─────────────────────────────────────────────────────────────
    if el_type >= 24.9 && el_type <= 25.1 {
        // Unpack time from progress: hr * 10000 + mn * 100 + sec
        let total_time_code = u32(progress);
        var hr = f32(total_time_code / 10000u);
        let rem_time = total_time_code % 10000u;
        let mn = f32(rem_time / 100u);
        let sec = f32(rem_time % 100u);
        
        let is_12h = (params.ior > 0.5);
        let is_pm = (hr >= 12.0);
        if is_12h {
            if hr == 0.0 { hr = 12.0; }
            else if hr > 12.0 { hr = hr - 12.0; }
        }
        
        // Unpack date from deformation: wday * 10000 + month * 100 + mday
        let total_date_code = u32(params.deformation);
        let wday_code = total_date_code / 10000u;
        let rem_date = total_date_code % 10000u;
        let month_code = rem_date / 100u;
        let mday_code = rem_date % 100u;
        
        let bat_val = clamp(params.roughness, 0.0, 100.0);
        
        // Dynamic Accent Adaptation:
        // If wallpaper has accent (tint.w > 0.5), use tint.xyz
        // Otherwise: pure neutral crystalline glass ("otherwise just no color")
        let has_accent = (tint.w > 0.5);
        let accent_raw = tint.xyz;
        let neutral_accent = mix(vec3<f32>(0.96, 0.98, 1.0), vec3<f32>(0.11, 0.14, 0.18), is_light);
        let primary_accent = select(neutral_accent, accent_raw, has_accent);
        
        // High-contrast adaptive typography
        let text_primary = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.08, 0.10, 0.14), is_light);
        let text_secondary = mix(vec3<f32>(0.90, 0.93, 0.98), vec3<f32>(0.22, 0.25, 0.30), is_light);
        let pill_frost_col = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.0, 0.0, 0.0), is_light);
        let pill_rim_col = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.08, 0.10, 0.14), is_light);

        // ════════════════════════════════════════════
        // 1. TOP ROW (y = -56.0): Frosted Day Pill & Battery Pill
        // ════════════════════════════════════════════
        let top_y = -56.0;
        
        // ── 1a. Left Day Pill (cx = -76.0, half = 52.0x13.0, r = 13.0) ──
        let p_day_pill = local_p - vec2<f32>(-76.0, top_y);
        let d_day_pill = rr_sdf(p_day_pill, vec2<f32>(52.0, 13.0), 13.0);
        let mask_day_fill = (1.0 - smoothstep(-0.4, 0.4, d_day_pill)) * 0.08;
        let mask_day_rim = smoothstep(1.1, 0.0, abs(d_day_pill)) * 0.25;
        
        // Status indicator dot (cx = -114.0, cy = top_y, r = 3.0)
        let p_dot = local_p - vec2<f32>(-114.0, top_y);
        let d_dot = length(p_dot) - 3.2;
        let mask_dot = 1.0 - smoothstep(-0.35, 0.35, d_dot);
        let dot_glow = exp(-max(d_dot, 0.0) * 0.45) * 0.75;
        
        // Day of Week Text: SUN, MON, TUE, WED, THU, FRI, SAT
        var w0 = 83u; var w1 = 85u; var w2 = 78u; // SUN
        if wday_code == 1u { w0 = 77u; w1 = 79u; w2 = 78u; } // MON
        else if wday_code == 2u { w0 = 84u; w1 = 85u; w2 = 69u; } // TUE
        else if wday_code == 3u { w0 = 87u; w1 = 69u; w2 = 68u; } // WED
        else if wday_code == 4u { w0 = 84u; w1 = 72u; w2 = 85u; } // THU
        else if wday_code == 5u { w0 = 70u; w1 = 82u; w2 = 73u; } // FRI
        else if wday_code == 6u { w0 = 83u; w1 = 65u; w2 = 84u; } // SAT
        
        let day_cx = -76.0;
        let dsp = 9.0;
        let d_w0 = draw_clock_letter(local_p - vec2<f32>(day_cx - 1.6 * dsp, top_y), w0, 1.15);
        let d_w1 = draw_clock_letter(local_p - vec2<f32>(day_cx - 0.5 * dsp, top_y), w1, 1.15);
        let d_w2 = draw_clock_letter(local_p - vec2<f32>(day_cx + 0.6 * dsp, top_y), w2, 1.15);
        let d_day_letters = min(min(d_w0, d_w1), d_w2) - 0.72;
        let mask_day_letters = 1.0 - smoothstep(-0.35, 0.35, d_day_letters);
        
        // ── 1b. Right Battery Pill (cx = 76.0, half = 46.0x13.0, r = 13.0) ──
        let p_bat_pill = local_p - vec2<f32>(76.0, top_y);
        let d_bat_pill = rr_sdf(p_bat_pill, vec2<f32>(46.0, 13.0), 13.0);
        let mask_bat_fill = (1.0 - smoothstep(-0.4, 0.4, d_bat_pill)) * 0.08;
        let mask_bat_rim = smoothstep(1.1, 0.0, abs(d_bat_pill)) * 0.25;
        
        // Lightning Bolt Icon (cx = 48.0, cy = top_y)
        let p_bolt = local_p - vec2<f32>(48.0, top_y);
        let s_b0 = dist_segment(p_bolt, vec2<f32>(1.2, -4.5), vec2<f32>(-1.8, -0.5));
        let s_b1 = dist_segment(p_bolt, vec2<f32>(-1.8, -0.5), vec2<f32>(0.8, -0.5));
        let s_b2 = dist_segment(p_bolt, vec2<f32>(0.8, -0.5), vec2<f32>(-1.2, 4.5));
        let d_bolt = min(min(s_b0, s_b1), s_b2) - 0.75;
        let mask_bolt = 1.0 - smoothstep(-0.35, 0.35, d_bolt);
        
        // Battery Percentage digits (No ugly leading zeros)
        let bat_u = u32(bat_val);
        let b_tens = (bat_u % 100u) / 10u;
        let b_ones = bat_u % 10u;
        let b_scale = 1.10;
        let b_sp = 7.5;
        let bat_num_cx = 76.0;
        var mask_bnum = 0.0;
        if bat_u >= 100u {
            let db_h = draw_digit(local_p - vec2<f32>(bat_num_cx - b_sp * 1.1, top_y), 1u, b_scale);
            let db0 = draw_digit(local_p - vec2<f32>(bat_num_cx, top_y), 0u, b_scale);
            let db1 = draw_digit(local_p - vec2<f32>(bat_num_cx + b_sp * 1.1, top_y), 0u, b_scale);
            let db_pct = draw_percent(local_p - vec2<f32>(bat_num_cx + b_sp * 2.3, top_y), b_scale);
            let d_all = min(min(db_h, db0), min(db1, db_pct)) - 0.68;
            mask_bnum = 1.0 - smoothstep(-0.35, 0.35, d_all);
        } else if bat_u >= 10u {
            let db0 = draw_digit(local_p - vec2<f32>(bat_num_cx - b_sp * 0.7, top_y), b_tens, b_scale);
            let db1 = draw_digit(local_p - vec2<f32>(bat_num_cx + b_sp * 0.5, top_y), b_ones, b_scale);
            let db_pct = draw_percent(local_p - vec2<f32>(bat_num_cx + b_sp * 1.8, top_y), b_scale);
            let d_all = min(min(db0, db1), db_pct) - 0.68;
            mask_bnum = 1.0 - smoothstep(-0.35, 0.35, d_all);
        } else {
            let db1 = draw_digit(local_p - vec2<f32>(bat_num_cx - b_sp * 0.2, top_y), b_ones, b_scale);
            let db_pct = draw_percent(local_p - vec2<f32>(bat_num_cx + b_sp * 1.1, top_y), b_scale);
            let d_all = min(db1, db_pct) - 0.68;
            mask_bnum = 1.0 - smoothstep(-0.35, 0.35, d_all);
        }

        // ════════════════════════════════════════════
        // 2. HERO DIGITAL CLOCK (y = -4.0): Bold Modern Typography
        // ════════════════════════════════════════════
        let time_y = -4.0;
        let time_cx = -12.0;
        let t_scale = 3.6;
        let sp = 23.5;
        
        let h_tens = u32(hr / 10.0);
        let h_ones = u32(hr % 10.0);
        let m_tens = u32(mn / 10.0);
        let m_ones = u32(mn % 10.0);
        
        let d_th0 = draw_digit(local_p - vec2<f32>(time_cx - 2.15 * sp, time_y), h_tens, t_scale);
        let d_th1 = draw_digit(local_p - vec2<f32>(time_cx - 1.05 * sp, time_y), h_ones, t_scale);
        let d_tcol = draw_colon(local_p - vec2<f32>(time_cx, time_y), t_scale);
        let d_tm0 = draw_digit(local_p - vec2<f32>(time_cx + 1.05 * sp, time_y), m_tens, t_scale);
        let d_tm1 = draw_digit(local_p - vec2<f32>(time_cx + 2.15 * sp, time_y), m_ones, t_scale);
        
        let d_time_digits = min(min(d_th0, d_th1), min(d_tm0, d_tm1)) - 1.80;
        let mask_time_digits = 1.0 - smoothstep(-0.35, 0.35, d_time_digits);
        
        let colon_pulse = 0.55 + 0.45 * sin(sec * 3.14159265);
        let mask_time_colon = (1.0 - smoothstep(-0.35, 0.35, d_tcol - 1.20)) * colon_pulse;
        
        // AM / PM Badge (scale = 0.95)
        let pm_x = time_cx + 2.15 * sp + 22.0;
        let pm_y = time_y + 4.0;
        let ch_ampm = select(65u, 80u, is_pm); // 'A' or 'P'
        let d_ampm0 = draw_clock_letter(local_p - vec2<f32>(pm_x, pm_y), ch_ampm, 0.95);
        let d_ampm1 = draw_clock_letter(local_p - vec2<f32>(pm_x + 7.5, pm_y), 77u, 0.95); // 'M'
        let mask_ampm = 1.0 - smoothstep(-0.35, 0.35, min(d_ampm0, d_ampm1) - 0.65);

        // ════════════════════════════════════════════
        // 3. BOTTOM ROW (y = 54.0): Frosted Date Pill & Battery Fill Track
        // ════════════════════════════════════════════
        let bot_y = 54.0;
        
        // ── 3a. Left Date Pill (cx = -76.0, half = 52.0x13.0, r = 13.0) ──
        let p_date_pill = local_p - vec2<f32>(-76.0, bot_y);
        let d_date_pill = rr_sdf(p_date_pill, vec2<f32>(52.0, 13.0), 13.0);
        let mask_date_fill = (1.0 - smoothstep(-0.4, 0.4, d_date_pill)) * 0.08;
        let mask_date_rim = smoothstep(1.1, 0.0, abs(d_date_pill)) * 0.25;
        
        // Calendar Icon (cx = -114.0, cy = bot_y)
        let p_cal = local_p - vec2<f32>(-114.0, bot_y);
        let d_cal_box = rr_sdf(p_cal, vec2<f32>(4.2, 4.0), 1.2);
        let mask_cal_outline = smoothstep(0.4, -0.4, abs(d_cal_box) - 0.75);
        let d_pin0 = dist_segment(p_cal, vec2<f32>(-2.2, -4.5), vec2<f32>(-2.2, -3.2)) - 0.6;
        let d_pin1 = dist_segment(p_cal, vec2<f32>(2.2, -4.5), vec2<f32>(2.2, -3.2)) - 0.6;
        let mask_cal_pins = 1.0 - smoothstep(-0.35, 0.35, min(d_pin0, d_pin1));
        let mask_cal_icon = max(mask_cal_outline, mask_cal_pins);
        
        // Month Letters (JAN..DEC)
        var m0 = 78u; var m1 = 79u; var m2 = 86u; // NOV default
        if month_code == 1u { m0 = 74u; m1 = 65u; m2 = 78u; }      // JAN
        else if month_code == 2u { m0 = 70u; m1 = 69u; m2 = 66u; } // FEB
        else if month_code == 3u { m0 = 77u; m1 = 65u; m2 = 82u; } // MAR
        else if month_code == 4u { m0 = 65u; m1 = 80u; m2 = 82u; } // APR
        else if month_code == 5u { m0 = 77u; m1 = 65u; m2 = 89u; } // MAY
        else if month_code == 6u { m0 = 74u; m1 = 85u; m2 = 78u; } // JUN
        else if month_code == 7u { m0 = 74u; m1 = 85u; m2 = 76u; } // JUL
        else if month_code == 8u { m0 = 65u; m1 = 85u; m2 = 71u; } // AUG
        else if month_code == 9u { m0 = 83u; m1 = 69u; m2 = 80u; } // SEP
        else if month_code == 10u { m0 = 79u; m1 = 67u; m2 = 84u; } // OCT
        else if month_code == 11u { m0 = 78u; m1 = 79u; m2 = 86u; } // NOV
        else if month_code == 12u { m0 = 68u; m1 = 69u; m2 = 67u; } // DEC
        
        let date_cx = -76.0;
        let mt_sp = 7.8;
        let d_m0 = draw_clock_letter(local_p - vec2<f32>(date_cx - 2.8 * mt_sp, bot_y), m0, 1.05);
        let d_m1 = draw_clock_letter(local_p - vec2<f32>(date_cx - 1.8 * mt_sp, bot_y), m1, 1.05);
        let d_m2 = draw_clock_letter(local_p - vec2<f32>(date_cx - 0.8 * mt_sp, bot_y), m2, 1.05);
        
        let md_tens = mday_code / 10u;
        let md_ones = mday_code % 10u;
        let d_d0 = draw_digit(local_p - vec2<f32>(date_cx + 0.9 * mt_sp, bot_y), md_tens, 1.05);
        let d_d1 = draw_digit(local_p - vec2<f32>(date_cx + 2.0 * mt_sp, bot_y), md_ones, 1.05);
        let d_date_text = min(min(min(d_m0, d_m1), d_m2), min(d_d0, d_d1)) - 0.70;
        let mask_date_text = 1.0 - smoothstep(-0.35, 0.35, d_date_text);
        
        // ── 3b. Right Battery Progress Track (cx = 76.0, half = 46.0x5.0, r = 5.0) ──
        let p_track = local_p - vec2<f32>(76.0, bot_y);
        let d_track = rr_sdf(p_track, vec2<f32>(46.0, 5.0), 5.0);
        let mask_track_bg = (1.0 - smoothstep(-0.4, 0.4, d_track)) * 0.08;
        let mask_track_rim = smoothstep(1.0, 0.0, abs(d_track)) * 0.25;
        
        let max_bar_w = 88.0;
        let bar_w = (bat_val / 100.0) * max_bar_w;
        let bar_cx = 32.0 + bar_w * 0.5;
        let d_bar_fill = rr_sdf(local_p - vec2<f32>(bar_cx, bot_y), vec2<f32>(max(bar_w * 0.5, 1.0), 3.5), 3.5);
        let mask_bar_fill = 1.0 - smoothstep(-0.35, 0.35, d_bar_fill);
        let bar_glow = exp(-max(d_bar_fill, 0.0) * 0.6) * 0.35;

        // ════════════════════════════════════════════
        // 4. MASTER COMPOSITE FOR CLOCK WIDGET
        // ════════════════════════════════════════════
        // Subtle ambient contact shadow cast beneath digits (+1.8px) for natural physical depth
        let p_digit_sh = local_p - vec2<f32>(0.0, 1.8);
        let d_sh0 = draw_digit(p_digit_sh - vec2<f32>(time_cx - 2.15 * sp, time_y), h_tens, t_scale);
        let d_sh1 = draw_digit(p_digit_sh - vec2<f32>(time_cx - 1.05 * sp, time_y), h_ones, t_scale);
        let d_sm0 = draw_digit(p_digit_sh - vec2<f32>(time_cx + 1.05 * sp, time_y), m_tens, t_scale);
        let d_sm1 = draw_digit(p_digit_sh - vec2<f32>(time_cx + 2.15 * sp, time_y), m_ones, t_scale);
        let d_time_sh = min(min(d_sh0, d_sh1), min(d_sm0, d_sm1)) - 1.80;
        let shadow_time = exp(-max(d_time_sh, 0.0) * 0.90) * 0.28;

        // Primary glyphs (Time digits, colon, day letters, date text, battery %)
        let primary_glyphs = max(
            max(mask_time_digits, mask_time_colon),
            max(max(mask_day_letters, mask_date_text), max(mask_bnum, max(mask_cal_icon, mask_ampm)))
        );

        let digit_sheen = clamp(1.0 - (local_p.y - (time_y - 12.0)) / 24.0, 0.0, 1.0);
        let glyph_light = mix(text_primary, vec3<f32>(1.0, 1.0, 1.0), digit_sheen * 0.20);

        // Inset Frosted Glass Pills (Day, Battery, Date, Battery Track)
        let pill_fills = max(max(mask_day_fill, mask_bat_fill), max(mask_date_fill, mask_track_bg));
        let pill_rims = max(max(mask_day_rim, mask_bat_rim), max(mask_date_rim, mask_track_rim));

        // Accent elements (Status dot, lightning bolt, battery liquid bar)
        let accent_glyphs = max(max(mask_dot, mask_bolt), mask_bar_fill);
        let total_glow = max(dot_glow, bar_glow);

        if accent_glyphs > 0.05 {
            app.glyph_mask = accent_glyphs;
            app.glyph_color = primary_accent;
            app.fluid_color = primary_accent;
            app.fluid_glow = total_glow;
        } else if primary_glyphs > 0.02 {
            app.glyph_mask = primary_glyphs;
            app.glyph_color = glyph_light;
        } else if shadow_time > 0.04 {
            app.glyph_mask = shadow_time;
            app.glyph_color = vec3<f32>(0.02, 0.03, 0.05);
        } else if pill_rims > 0.05 {
            app.glyph_mask = pill_rims;
            app.glyph_color = pill_rim_col;
        } else if pill_fills > 0.05 {
            app.glyph_mask = pill_fills;
            app.glyph_color = pill_frost_col;
        }
        return app;
    }

    // ─────────────────────────────────────────────────────────────
    // 26. Apple Liquid Glass Todo & Calendar Widget (360x455)
    // ─────────────────────────────────────────────────────────────
    if el_type >= 25.5 && el_type <= 26.5 {
        let has_accent = (tint.w > 0.5);
        let accent_raw = tint.xyz;
        let neutral_accent = mix(vec3<f32>(0.96, 0.98, 1.0), vec3<f32>(0.11, 0.14, 0.18), is_light);
        let primary_accent = select(neutral_accent, accent_raw, has_accent);

        let text_primary = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.08, 0.10, 0.14), is_light);
        let text_dim = mix(vec3<f32>(0.65, 0.68, 0.75), vec3<f32>(0.45, 0.48, 0.52), is_light);
        let text_secondary = mix(vec3<f32>(0.88, 0.91, 0.96), vec3<f32>(0.22, 0.25, 0.30), is_light);
        let pill_frost_col = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.0, 0.0, 0.0), is_light);
        let pill_rim_col = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.08, 0.10, 0.14), is_light);

        let mask_done = u32(progress);
        let hover_row = params.roughness;

        // ════════════════════════════════════════════
        // 1. TOP HEADER SEGMENTED TABS (y = -188.0)
        // ════════════════════════════════════════════
        let top_y = -188.0;
        let p_track = local_p - vec2<f32>(0.0, top_y);
        let d_track = rr_sdf(p_track, vec2<f32>(156.0, 16.0), 16.0);
        let mask_track_fill = (1.0 - smoothstep(-0.4, 0.4, d_track)) * 0.05;
        let mask_track_rim = smoothstep(1.0, 0.0, abs(d_track)) * 0.20;

        // Tab 1: Tasks (cx = -76.0, half = 72.0x13.0, r = 13.0) - ACTIVE
        let p_tab1 = local_p - vec2<f32>(-76.0, top_y);
        let d_tab1 = rr_sdf(p_tab1, vec2<f32>(72.0, 13.0), 13.0);
        let mask_tab1_fill = (1.0 - smoothstep(-0.4, 0.4, d_tab1)) * 0.10;
        let mask_tab1_rim = smoothstep(1.1, 0.0, abs(d_tab1)) * 0.28;

        // Tab 1 Text: TASKS (6)
        let t1_cx = -76.0;
        let t1_sp = 7.0;
        let dt0 = draw_clock_letter(local_p - vec2<f32>(t1_cx - 3.8 * t1_sp, top_y), 84u, 0.95); // T
        let dt1 = draw_clock_letter(local_p - vec2<f32>(t1_cx - 2.6 * t1_sp, top_y), 65u, 0.95); // A
        let dt2 = draw_clock_letter(local_p - vec2<f32>(t1_cx - 1.4 * t1_sp, top_y), 83u, 0.95); // S
        let dt3 = draw_clock_letter(local_p - vec2<f32>(t1_cx - 0.2 * t1_sp, top_y), 75u, 0.95); // K
        let dt4 = draw_clock_letter(local_p - vec2<f32>(t1_cx + 1.0 * t1_sp, top_y), 83u, 0.95); // S
        let dt_p0 = draw_clock_letter(local_p - vec2<f32>(t1_cx + 2.3 * t1_sp, top_y), 40u, 0.95); // (
        let dt5 = draw_digit(local_p - vec2<f32>(t1_cx + 3.3 * t1_sp, top_y), 6u, 0.95);
        let dt_p1 = draw_clock_letter(local_p - vec2<f32>(t1_cx + 4.3 * t1_sp, top_y), 41u, 0.95); // )
        let d_tab1_txt = min(min(min(dt0, dt1), min(dt2, dt3)), min(min(dt4, dt_p0), min(dt5, dt_p1))) - 0.70;
        let mask_tab1_txt = 1.0 - smoothstep(-0.35, 0.35, d_tab1_txt);

        // Tab 2: Agenda (cx = 76.0, half = 72.0x13.0, r = 13.0) - INACTIVE
        let p_tab2 = local_p - vec2<f32>(76.0, top_y);
        let d_tab2 = rr_sdf(p_tab2, vec2<f32>(72.0, 13.0), 13.0);
        let mask_tab2_rim = smoothstep(1.0, 0.0, abs(d_tab2)) * mix(0.25, 0.15, is_light);

        // Tab 2 Text: AGENDA
        let t2_cx = 76.0;
        let da0 = draw_clock_letter(local_p - vec2<f32>(t2_cx - 2.6 * t1_sp, top_y), 65u, 0.90); // A
        let da1 = draw_clock_letter(local_p - vec2<f32>(t2_cx - 1.5 * t1_sp, top_y), 71u, 0.90); // G
        let da2 = draw_clock_letter(local_p - vec2<f32>(t2_cx - 0.4 * t1_sp, top_y), 69u, 0.90); // E
        let da3 = draw_clock_letter(local_p - vec2<f32>(t2_cx + 0.7 * t1_sp, top_y), 78u, 0.90); // N
        let da4 = draw_clock_letter(local_p - vec2<f32>(t2_cx + 1.8 * t1_sp, top_y), 68u, 0.90); // D
        let da5 = draw_clock_letter(local_p - vec2<f32>(t2_cx + 2.9 * t1_sp, top_y), 65u, 0.90); // A
        let d_tab2_txt = min(min(min(da0, da1), min(da2, da3)), min(da4, da5)) - 0.65;
        let mask_tab2_txt = 1.0 - smoothstep(-0.35, 0.35, d_tab2_txt);

        // ════════════════════════════════════════════
        // 2. FILTER STRIP (y = -148.0)
        // ════════════════════════════════════════════
        let filt_y = -148.0;
        // ALL Pill
        let p_all = local_p - vec2<f32>(-96.0, filt_y);
        let d_all = rr_sdf(p_all, vec2<f32>(30.0, 10.0), 10.0);
        let mask_all_fill = (1.0 - smoothstep(-0.4, 0.4, d_all)) * 0.08;
        let mask_all_rim = smoothstep(1.0, 0.0, abs(d_all)) * 0.22;
        let dl0 = draw_clock_letter(p_all - vec2<f32>(-11.0, 0.0), 65u, 0.85); // A
        let dl1 = draw_clock_letter(p_all - vec2<f32>(0.0, 0.0), 76u, 0.85);  // L
        let dl2 = draw_clock_letter(p_all - vec2<f32>(11.0, 0.0), 76u, 0.85); // L
        let mask_all_txt = 1.0 - smoothstep(-0.35, 0.35, min(min(dl0, dl1), dl2) - 0.65);

        // ACTIVE Pill
        let p_act = local_p - vec2<f32>(-24.0, filt_y);
        let d_act = rr_sdf(p_act, vec2<f32>(34.0, 10.0), 10.0);
        let mask_act_rim = smoothstep(1.0, 0.0, abs(d_act)) * 0.16;
        let dc0 = draw_clock_letter(p_act - vec2<f32>(-16.0, 0.0), 65u, 0.80);
        let dc1 = draw_clock_letter(p_act - vec2<f32>(-6.0, 0.0), 67u, 0.80);
        let dc2 = draw_clock_letter(p_act - vec2<f32>(4.0, 0.0), 84u, 0.80);
        let dc3 = draw_clock_letter(p_act - vec2<f32>(14.0, 0.0), 86u, 0.80);
        let mask_act_txt = 1.0 - smoothstep(-0.35, 0.35, min(min(dc0, dc1), min(dc2, dc3)) - 0.60);

        // DONE Pill
        let p_don = local_p - vec2<f32>(48.0, filt_y);
        let d_don = rr_sdf(p_don, vec2<f32>(30.0, 10.0), 10.0);
        let mask_don_rim = smoothstep(1.0, 0.0, abs(d_don)) * 0.16;
        let dd0 = draw_clock_letter(p_don - vec2<f32>(-11.0, 0.0), 68u, 0.80);
        let dd1 = draw_clock_letter(p_don - vec2<f32>(0.0, 0.0), 79u, 0.80);
        let dd2 = draw_clock_letter(p_don - vec2<f32>(11.0, 0.0), 78u, 0.80);
        let mask_don_txt = 1.0 - smoothstep(-0.35, 0.35, min(min(dd0, dd1), dd2) - 0.60);

        // Refresh Icon ↻
        let p_ref = local_p - vec2<f32>(120.0, filt_y);
        let r_ref = length(p_ref);
        let d_ref_arc = abs(r_ref - 5.5) - 0.65;
        let ang_ref = atan2(p_ref.y, p_ref.x);
        let mask_ref_arc = select(1.0 - smoothstep(-0.35, 0.35, d_ref_arc), 0.0, ang_ref > 1.2 && ang_ref < 2.0);
        let d_ref_arr = dist_segment(p_ref, vec2<f32>(3.5, 3.5), vec2<f32>(7.5, 3.5));
        let mask_ref = max(mask_ref_arc, 1.0 - smoothstep(-0.35, 0.35, d_ref_arr - 0.65));

        // ════════════════════════════════════════════
        // 3. TASK ROWS (6 Rows, y = -105.0 to +115.0)
        // ════════════════════════════════════════════
        var rows_fill = 0.0;
        var rows_rim = 0.0;
        var check_fills = 0.0;
        var check_rims = 0.0;
        var check_marks = 0.0;
        var strikes = 0.0;
        var daily_fills = 0.0;
        var daily_rims = 0.0;
        var daily_txts = 0.0;
        var task_txt_primary = 0.0;
        var task_txt_shadow = 0.0;
        var task_txt_dimmed = 0.0;

        for (var i = 0u; i < 6u; i += 1u) {
            let row_y = -105.0 + f32(i) * 44.0;
            let p_row = local_p - vec2<f32>(0.0, row_y);
            let d_row = rr_sdf(p_row, vec2<f32>(156.0, 18.0), 10.0);

            let is_row_hover = (hover_row >= f32(i) - 0.1 && hover_row <= f32(i) + 0.1);
            let hover_boost = select(0.0, 0.12, is_row_hover);

            let fill = (1.0 - smoothstep(-0.4, 0.4, d_row)) * (0.06 + hover_boost);
            let rim = smoothstep(1.0, 0.0, abs(d_row)) * (0.18 + hover_boost);
            rows_fill = max(rows_fill, fill);
            rows_rim = max(rows_rim, rim);

            let is_item_done = ((mask_done & (1u << i)) != 0u);

            // Checkbox (cx = -135.0, cy = row_y)
            let p_box = local_p - vec2<f32>(-135.0, row_y);
            let d_box = rr_sdf(p_box, vec2<f32>(8.5, 8.5), 4.5);
            let c_rim = smoothstep(1.2, 0.0, abs(d_box)) * 0.32;
            let c_bg = (1.0 - smoothstep(-0.4, 0.4, d_box)) * 0.08;
            check_rims = max(check_rims, max(c_rim, c_bg));

            if is_item_done {
                let c_fill = (1.0 - smoothstep(-0.4, 0.4, d_box));
                check_fills = max(check_fills, c_fill);
                let d_cm = draw_checkmark(p_box, 1.05);
                check_marks = max(check_marks, 1.0 - smoothstep(-0.35, 0.35, d_cm));

                // Strikethrough horizontal line running through task title
                let d_str = dist_segment(local_p, vec2<f32>(-116.0, row_y), vec2<f32>(60.0, row_y)) - 0.85;
                strikes = max(strikes, 1.0 - smoothstep(-0.35, 0.35, d_str));
            }

            // Daily Tag Badge (cx = 124.0, cy = row_y, half = 22.0x7.5, r = 7.5)
            let p_dtag = local_p - vec2<f32>(124.0, row_y);
            let d_dtag = rr_sdf(p_dtag, vec2<f32>(22.0, 7.5), 7.5);
            daily_fills = max(daily_fills, (1.0 - smoothstep(-0.4, 0.4, d_dtag)) * 0.06);
            daily_rims = max(daily_rims, smoothstep(1.0, 0.0, abs(d_dtag)) * 0.20);

            let dy0 = draw_clock_letter(p_dtag - vec2<f32>(-12.0, 0.0), 68u, 0.65); // D
            let dy1 = draw_clock_letter(p_dtag - vec2<f32>(-6.0, 0.0), 65u, 0.65);  // A
            let dy2 = draw_clock_letter(p_dtag - vec2<f32>(0.0, 0.0), 73u, 0.65);   // I
            let dy3 = draw_clock_letter(p_dtag - vec2<f32>(6.0, 0.0), 76u, 0.65);   // L
            let dy4 = draw_clock_letter(p_dtag - vec2<f32>(12.0, 0.0), 89u, 0.65);  // Y
            let d_dytxt = min(min(min(dy0, dy1), min(dy2, dy3)), dy4) - 0.48;
            daily_txts = max(daily_txts, 1.0 - smoothstep(-0.35, 0.35, d_dytxt));

            // Task Titles with crisp bold strokes and drop shadow:
            let d_txt = eval_todo_row_text(local_p, i);
            let d_txt_sh = eval_todo_row_text(local_p - vec2<f32>(0.0, 1.2), i);

            let m_txt = 1.0 - smoothstep(-0.35, 0.35, d_txt - 0.78);
            let m_txt_sh = (1.0 - smoothstep(-0.35, 0.75, d_txt_sh - 0.78)) * 0.55;

            if is_item_done {
                task_txt_dimmed = max(task_txt_dimmed, m_txt);
            } else {
                task_txt_primary = max(task_txt_primary, m_txt);
                task_txt_shadow = max(task_txt_shadow, m_txt_sh);
            }
        }

        // ════════════════════════════════════════════
        // 4. BOTTOM ADD BAR (y = +185.0)
        // ════════════════════════════════════════════
        let bot_y = 185.0;
        let p_add_bar = local_p - vec2<f32>(0.0, bot_y);
        let d_add_bar = rr_sdf(p_add_bar, vec2<f32>(156.0, 17.0), 17.0);
        let mask_add_fill = (1.0 - smoothstep(-0.4, 0.4, d_add_bar)) * mix(0.12, 0.08, is_light);
        let mask_add_rim = smoothstep(1.1, 0.0, abs(d_add_bar)) * mix(0.35, 0.22, is_light);

        // Add Button '+' at cx = 135.0, cy = bot_y
        let p_plus = local_p - vec2<f32>(135.0, bot_y);
        let d_plus_bg = length(p_plus) - 11.0;
        let mask_plus_fill = 1.0 - smoothstep(-0.4, 0.4, d_plus_bg);
        let d_plus_v = dist_segment(p_plus, vec2<f32>(0.0, -5.0), vec2<f32>(0.0, 5.0));
        let d_plus_h = dist_segment(p_plus, vec2<f32>(-5.0, 0.0), vec2<f32>(5.0, 0.0));
        let mask_plus_icon = 1.0 - smoothstep(-0.35, 0.35, min(d_plus_v, d_plus_h) - 0.65);

        // Placeholder text: + ADD TASK...
        let at_start = -135.0;
        let at_sp = 7.5;
        let ap0 = draw_clock_letter(local_p - vec2<f32>(at_start + 0.0 * at_sp, bot_y), 65u, 0.85); // A
        let ap1 = draw_clock_letter(local_p - vec2<f32>(at_start + 1.0 * at_sp, bot_y), 68u, 0.85); // D
        let ap2 = draw_clock_letter(local_p - vec2<f32>(at_start + 2.0 * at_sp, bot_y), 68u, 0.85); // D
        let ap3 = draw_clock_letter(local_p - vec2<f32>(at_start + 3.8 * at_sp, bot_y), 84u, 0.85); // T
        let ap4 = draw_clock_letter(local_p - vec2<f32>(at_start + 4.8 * at_sp, bot_y), 65u, 0.85); // A
        let ap5 = draw_clock_letter(local_p - vec2<f32>(at_start + 5.8 * at_sp, bot_y), 83u, 0.85); // S
        let ap6 = draw_clock_letter(local_p - vec2<f32>(at_start + 6.8 * at_sp, bot_y), 75u, 0.85); // K
        let d_add_txt = min(min(min(ap0, ap1), ap2), min(min(ap3, ap4), min(ap5, ap6))) - 0.65;
        let mask_add_txt = 1.0 - smoothstep(-0.35, 0.35, d_add_txt);

        // ════════════════════════════════════════════
        // 5. MASTER COMPOSITE FOR TODO WIDGET
        // ════════════════════════════════════════════
        let all_rims = max(max(mask_track_rim, mask_tab1_rim), max(max(rows_rim, check_rims), max(daily_rims, mask_add_rim)));
        let all_fills = max(max(mask_track_fill, mask_tab1_fill), max(max(rows_fill, daily_fills), mask_add_fill));

        let primary_glyphs = max(
            max(mask_tab1_txt, mask_all_txt),
            max(max(task_txt_primary, check_marks), max(daily_txts, mask_plus_icon))
        );
        let secondary_glyphs = max(max(mask_tab2_txt, mask_act_txt), max(mask_don_txt, mask_add_txt));

        if mask_plus_icon > 0.05 {
            app.glyph_mask = mask_plus_icon;
            app.glyph_color = vec3<f32>(1.0, 1.0, 1.0);
        } else if check_marks > 0.05 {
            app.glyph_mask = check_marks;
            app.glyph_color = vec3<f32>(1.0, 1.0, 1.0);
        } else if check_fills > 0.05 {
            app.glyph_mask = check_fills;
            app.glyph_color = primary_accent;
            app.fluid_color = primary_accent;
            app.fluid_glow = 0.45;
        } else if mask_plus_fill > 0.05 {
            app.glyph_mask = mask_plus_fill;
            app.glyph_color = primary_accent;
            app.fluid_color = primary_accent;
            app.fluid_glow = 0.45;
        } else if strikes > 0.05 {
            app.glyph_mask = strikes;
            app.glyph_color = text_dim;
        } else if primary_glyphs > 0.02 {
            app.glyph_mask = primary_glyphs;
            app.glyph_color = text_primary;
        } else if task_txt_shadow > 0.03 {
            app.glyph_mask = task_txt_shadow;
            app.glyph_color = vec3<f32>(0.02, 0.03, 0.05);
        } else if task_txt_dimmed > 0.02 {
            app.glyph_mask = task_txt_dimmed;
            app.glyph_color = text_dim;
        } else if secondary_glyphs > 0.02 {
            app.glyph_mask = secondary_glyphs;
            app.glyph_color = text_secondary;
        } else if all_rims > 0.05 {
            app.glyph_mask = all_rims;
            app.glyph_color = pill_rim_col;
        } else if all_fills > 0.05 {
            app.glyph_mask = all_fills;
            app.glyph_color = pill_frost_col;
        }
        return app;
    }

    // ─────────────────────────────────────────────────────────────
    // 30. Minimalist Liquid Glass Square Box with Center Action Button
    // ─────────────────────────────────────────────────────────────
    // ─────────────────────────────────────────────────────────────
    // 30. Minimalist Liquid Glass Square Box with Center Action Button
    // ─────────────────────────────────────────────────────────────
    if el_type >= 29.5 && el_type <= 30.5 {
        let is_toggled = (tint.x > 0.5);
        let is_pressed = (tint.y > 0.5);
        let hover_amt = tint.z; // 0.0 - 1.0 spring
        
        // Button tactile scale: slight expansion on hover (+4%), slight compression on press (-5%)
        let btn_scale = select(1.0 + hover_amt * 0.04, 0.94, is_pressed);
        let p_btn = local_p / btn_scale;
        
        let btn_r = 36.0;
        let d_btn = length(p_btn) - btn_r;
        
        // Polished glass button face (subtle frosted translucent glass disc)
        let btn_fill = (1.0 - smoothstep(-0.8, 0.8, d_btn)) * (0.18 + hover_amt * 0.14);
        
        // Button surface specular sunlight catch
        let btn_dir = normalize(p_btn + vec2<f32>(0.001));
        let btn_sun = max(dot(btn_dir, vec2<f32>(-0.6, -0.8)), 0.0);
        let btn_rim = smoothstep(1.4, 0.0, abs(d_btn)) * (btn_sun * 0.65 + 0.30);
        
        // Center Icon: Toggle between Play (triangle) and Pause (two vertical batons)
        var mask_icon = 0.0;
        if is_toggled {
            // Pause: two sleek rounded vertical batons
            let b0 = rr_sdf(p_btn - vec2<f32>(-6.5, 0.0), vec2<f32>(3.0, 11.0), 2.0);
            let b1 = rr_sdf(p_btn - vec2<f32>(6.5, 0.0), vec2<f32>(3.0, 11.0), 2.0);
            let d_pause = min(b0, b1);
            mask_icon = 1.0 - smoothstep(-0.4, 0.4, d_pause);
        } else {
            // Play: precision rounded equilateral triangle (with optical center offset)
            let q = p_btn - vec2<f32>(2.5, 0.0);
            let e0 = dist_segment(q, vec2<f32>(-8.5, -12.5), vec2<f32>(-8.5, 12.5));
            let e1 = dist_segment(q, vec2<f32>(-8.5, 12.5), vec2<f32>(13.5, 0.0));
            let e2 = dist_segment(q, vec2<f32>(13.5, 0.0), vec2<f32>(-8.5, -12.5));
            let tri_edge = min(min(e0, e1), e2);
            let is_in = (q.x >= -8.5) && (q.x * 1.15 + abs(q.y) * 1.4 <= 15.5);
            let d_play = select(tri_edge, -tri_edge, is_in);
            mask_icon = 1.0 - smoothstep(-0.4, 0.4, d_play);
        }

        let white = mix(vec3<f32>(0.98, 0.99, 1.0), vec3<f32>(0.11, 0.14, 0.18), is_light);

        // Pure crisp icon mask (no opaque white blob filling the button)
        app.glyph_mask = mask_icon;
        app.glyph_color = white;

        // Button glass surface specular glint and ethereal hover liquid glow
        let btn_spec = btn_rim * 0.65 + btn_fill * 0.15;
        app.fluid_color = mix(white, vec3<f32>(0.40, 0.82, 1.0), hover_amt);
        app.fluid_glow = btn_spec + exp(-max(d_btn, 0.0) * 0.16) * hover_amt * 0.35 * (1.0 - is_light * 0.5);

        return app;
    }
    

    // ─────────────────────────────────────────────────────────────
    // 35. Liquid Glass Media Control Island (420px x 110px capsule)
    // ─────────────────────────────────────────────────────────────
    if el_type >= 34.5 && el_type <= 35.5 {
        let is_playing = (tint.x > 0.5);
        let prog = clamp(tint.y, 0.0, 1.0);
        let hover_play = tint.z; // 0.0 - 1.0
        let elapsed_sec = tint.w;
        let is_pressed = (progress % 10.0 > 0.5);
        
        // ── 1. Vinyl Album Disc with Music Note (cx = -135.0, cy = -4.0, r = 28.0) ──
        let p_vinyl = local_p - vec2<f32>(-135.0, -4.0);
        let r_vinyl = length(p_vinyl);
        
        let d_disc = r_vinyl - 28.0;
        let mask_disc_edge = smoothstep(0.4, -0.4, abs(d_disc) - 0.65);
        let mask_disc_fill = (1.0 - smoothstep(-0.5, 0.5, d_disc)) * 0.16;
        
        // Concentric vinyl grooves
        var grooves = 0.0;
        if r_vinyl >= 11.5 && r_vinyl <= 26.5 {
            grooves = (sin(r_vinyl * 2.8) * 0.5 + 0.5) * 0.22;
        }
        
        // Center label disc (r = 10.5)
        let d_label = r_vinyl - 10.5;
        let mask_label = (1.0 - smoothstep(-0.4, 0.4, d_label)) * 0.28;
        let mask_spindle = 1.0 - smoothstep(-0.35, 0.35, r_vinyl - 2.8);
        
        // Rotating center music note (♫)
        let rot_speed = select(0.0, time * 2.2, is_playing);
        let p_rot = vec2<f32>(cos(rot_speed) * p_vinyl.x - sin(rot_speed) * p_vinyl.y, 
                              sin(rot_speed) * p_vinyl.x + cos(rot_speed) * p_vinyl.y);
                              
        let d_n1 = length(p_rot - vec2<f32>(-3.2, 2.8)) - 2.0;
        let d_n2 = length(p_rot - vec2<f32>(3.2, 1.2)) - 2.0;
        let d_st1 = dist_segment(p_rot, vec2<f32>(-1.6, 2.8), vec2<f32>(-1.6, -3.8)) - 0.65;
        let d_st2 = dist_segment(p_rot, vec2<f32>(4.8, 1.2), vec2<f32>(4.8, -5.4)) - 0.65;
        let d_bm = dist_segment(p_rot, vec2<f32>(-1.6, -3.8), vec2<f32>(4.8, -5.4)) - 1.0;
        let d_note = min(min(d_n1, d_n2), min(min(d_st1, d_st2), d_bm));
        let mask_note = 1.0 - smoothstep(-0.35, 0.35, d_note);
        
        // ── 2. Equalizer Audio Wave Visualizer (-92.0 to -42.0, y = -4.0) ──
        var eq_mask = 0.0;
        for (var i = 0; i < 5; i++) {
            let bx = -92.0 + f32(i) * 11.0;
            let phase = f32(i) * 1.35;
            let bar_h = select(2.8, 3.5 + 8.5 * abs(sin(time * 3.8 + phase)), is_playing);
            let d_bar = rr_sdf(local_p - vec2<f32>(bx, -4.0), vec2<f32>(1.8, bar_h), 1.2);
            eq_mask = max(eq_mask, 1.0 - smoothstep(-0.35, 0.35, d_bar));
        }
        
        // ── 3. Subtle Vertical Crystalline Divider (x = -26.0, y = -4.0) ──
        let d_sep = rr_sdf(local_p - vec2<f32>(-26.0, -4.0), vec2<f32>(0.5, 20.0), 0.5);
        let mask_sep = (1.0 - smoothstep(-0.35, 0.35, d_sep)) * 0.30;
        
        // ── 4. Transport Controls: Prev (x = 4.0), Play (x = 48.0), Next (x = 92.0) at y = -10.0 ──
        let y_ctrl = -10.0;
        
        // Previous (|<<) at x = 4.0
        let p_prev = local_p - vec2<f32>(4.0, y_ctrl);
        let bar_prev = dist_segment(p_prev, vec2<f32>(-6.5, -5.5), vec2<f32>(-6.5, 5.5)) - 0.85;
        // Two left-pointing triangles
        let q_pr1 = p_prev - vec2<f32>(-2.0, 0.0);
        let e_pr1_0 = dist_segment(q_pr1, vec2<f32>(3.5, -5.5), vec2<f32>(3.5, 5.5));
        let e_pr1_1 = dist_segment(q_pr1, vec2<f32>(3.5, 5.5), vec2<f32>(-3.5, 0.0));
        let e_pr1_2 = dist_segment(q_pr1, vec2<f32>(-3.5, 0.0), vec2<f32>(3.5, -5.5));
        let in_pr1 = (q_pr1.x <= 3.5) && (q_pr1.x * -1.0 + abs(q_pr1.y) * 1.2 <= 4.2);
        let d_tri_pr1 = select(min(min(e_pr1_0, e_pr1_1), e_pr1_2), -min(min(e_pr1_0, e_pr1_1), e_pr1_2), in_pr1) - 0.4;
        
        let q_pr2 = p_prev - vec2<f32>(4.5, 0.0);
        let e_pr2_0 = dist_segment(q_pr2, vec2<f32>(3.5, -5.5), vec2<f32>(3.5, 5.5));
        let e_pr2_1 = dist_segment(q_pr2, vec2<f32>(3.5, 5.5), vec2<f32>(-3.5, 0.0));
        let e_pr2_2 = dist_segment(q_pr2, vec2<f32>(-3.5, 0.0), vec2<f32>(3.5, -5.5));
        let in_pr2 = (q_pr2.x <= 3.5) && (q_pr2.x * -1.0 + abs(q_pr2.y) * 1.2 <= 4.2);
        let d_tri_pr2 = select(min(min(e_pr2_0, e_pr2_1), e_pr2_2), -min(min(e_pr2_0, e_pr2_1), e_pr2_2), in_pr2) - 0.4;
        let mask_prev = 1.0 - smoothstep(-0.4, 0.4, min(bar_prev, min(d_tri_pr1, d_tri_pr2)));
        
        // Next (>>|) at x = 92.0
        let p_next = local_p - vec2<f32>(92.0, y_ctrl);
        let bar_next = dist_segment(p_next, vec2<f32>(6.5, -5.5), vec2<f32>(6.5, 5.5)) - 0.85;
        // Two right-pointing triangles
        let q_nx1 = p_next - vec2<f32>(-4.5, 0.0);
        let e_nx1_0 = dist_segment(q_nx1, vec2<f32>(-3.5, -5.5), vec2<f32>(-3.5, 5.5));
        let e_nx1_1 = dist_segment(q_nx1, vec2<f32>(-3.5, 5.5), vec2<f32>(3.5, 0.0));
        let e_nx1_2 = dist_segment(q_nx1, vec2<f32>(3.5, 0.0), vec2<f32>(-3.5, -5.5));
        let in_nx1 = (q_nx1.x >= -3.5) && (q_nx1.x * 1.0 + abs(q_nx1.y) * 1.2 <= 4.2);
        let d_tri_nx1 = select(min(min(e_nx1_0, e_nx1_1), e_nx1_2), -min(min(e_nx1_0, e_nx1_1), e_nx1_2), in_nx1) - 0.4;
        
        let q_nx2 = p_next - vec2<f32>(2.0, 0.0);
        let e_nx2_0 = dist_segment(q_nx2, vec2<f32>(-3.5, -5.5), vec2<f32>(-3.5, 5.5));
        let e_nx2_1 = dist_segment(q_nx2, vec2<f32>(-3.5, 5.5), vec2<f32>(3.5, 0.0));
        let e_nx2_2 = dist_segment(q_nx2, vec2<f32>(3.5, 0.0), vec2<f32>(-3.5, -5.5));
        let in_nx2 = (q_nx2.x >= -3.5) && (q_nx2.x * 1.0 + abs(q_nx2.y) * 1.2 <= 4.2);
        let d_tri_nx2 = select(min(min(e_nx2_0, e_nx2_1), e_nx2_2), -min(min(e_nx2_0, e_nx2_1), e_nx2_2), in_nx2) - 0.4;
        let mask_next = 1.0 - smoothstep(-0.4, 0.4, min(bar_next, min(d_tri_nx1, d_tri_nx2)));
        
        // Hero Play / Pause Button at x = 48.0
        let btn_scale = select(1.0 + hover_play * 0.05, 0.94, is_pressed);
        let p_btn = (local_p - vec2<f32>(48.0, y_ctrl)) / btn_scale;
        let btn_r = 21.0;
        let d_btn = length(p_btn) - btn_r;
        let btn_fill = (1.0 - smoothstep(-0.8, 0.8, d_btn)) * (0.22 + hover_play * 0.16);
        let btn_dir = normalize(p_btn + vec2<f32>(0.001));
        let btn_sun = max(dot(btn_dir, vec2<f32>(-0.6, -0.8)), 0.0);
        let btn_rim = smoothstep(1.3, 0.0, abs(d_btn)) * (btn_sun * 0.70 + 0.35);
        
        var mask_play_icon = 0.0;
        if is_playing {
            // Pause: two sleek rounded vertical batons
            let b0 = rr_sdf(p_btn - vec2<f32>(-4.2, 0.0), vec2<f32>(2.2, 7.5), 1.6);
            let b1 = rr_sdf(p_btn - vec2<f32>(4.2, 0.0), vec2<f32>(2.2, 7.5), 1.6);
            let d_pause = min(b0, b1);
            mask_play_icon = 1.0 - smoothstep(-0.35, 0.35, d_pause);
        } else {
            // Play: precision equilateral triangle with optical center offset
            let q_pl = p_btn - vec2<f32>(1.8, 0.0);
            let e0 = dist_segment(q_pl, vec2<f32>(-5.5, -8.0), vec2<f32>(-5.5, 8.0));
            let e1 = dist_segment(q_pl, vec2<f32>(-5.5, 8.0), vec2<f32>(9.0, 0.0));
            let e2 = dist_segment(q_pl, vec2<f32>(9.0, 0.0), vec2<f32>(-5.5, -8.0));
            let is_in = (q_pl.x >= -5.5) && (q_pl.x * 1.1 + abs(q_pl.y) * 1.3 <= 10.5);
            let d_pl = select(min(min(e0, e1), e2), -min(min(e0, e1), e2), is_in) - 0.4;
            mask_play_icon = 1.0 - smoothstep(-0.35, 0.35, d_pl);
        }
        
        // ── 5. Timeline Progress Bar Track & Scrubber Knob (y = 22.0) ──
        let y_bar = 22.0;
        // Total track spans from x = -15.0 to x = 135.0 (width = 150.0)
        let bar_len = 150.0;
        let d_rail = rr_sdf(local_p - vec2<f32>(60.0, y_bar), vec2<f32>(75.0, 1.8), 1.4);
        let mask_rail = (1.0 - smoothstep(-0.35, 0.35, d_rail)) * 0.28;
        
        let fill_w = prog * bar_len;
        let fill_cx = -15.0 + fill_w * 0.5;
        let d_fill = rr_sdf(local_p - vec2<f32>(fill_cx, y_bar), vec2<f32>(max(fill_w * 0.5, 1.0), 2.0), 1.4);
        let mask_fill = 1.0 - smoothstep(-0.35, 0.35, d_fill);
        
        // Scrubber Knob (circle r = 4.2)
        let p_knob = local_p - vec2<f32>(-15.0 + fill_w, y_bar);
        let d_knob = length(p_knob) - 4.5;
        let mask_knob = 1.0 - smoothstep(-0.35, 0.35, d_knob);
        
        // ── 6. Elapsed Time Display (x = -40.0, y = 22.0) ──
        let cur_sec = u32(elapsed_sec);
        let e_min = (cur_sec / 60u) % 10u;
        let e_s_tens = (cur_sec % 60u) / 10u;
        let e_s_ones = cur_sec % 10u;
        let t_scale = 1.05;
        let tsp = 7.2;
        let d_em = draw_digit(local_p - vec2<f32>(-50.0 - tsp, y_bar), e_min, t_scale);
        let d_ecol = draw_colon(local_p - vec2<f32>(-50.0, y_bar), t_scale);
        let d_es0 = draw_digit(local_p - vec2<f32>(-50.0 + tsp * 0.9, y_bar), e_s_tens, t_scale);
        let d_es1 = draw_digit(local_p - vec2<f32>(-50.0 + tsp * 1.9, y_bar), e_s_ones, t_scale);
        let d_time_text = min(min(d_em, d_ecol), min(d_es0, d_es1)) - 0.42;
        let mask_time_text = (1.0 - smoothstep(-0.35, 0.35, d_time_text)) * 0.85;
        
        // ── 7. Volume Speaker Icon & Soundwaves (x = 162.0, y = y_ctrl) ──
        let p_spk = local_p - vec2<f32>(160.0, y_ctrl);
        // Speaker box: 4px x 6px
        let d_spk_box = rr_sdf(p_spk - vec2<f32>(-4.0, 0.0), vec2<f32>(2.0, 3.0), 0.8);
        // Speaker cone: triangle
        let d_spk_cone = dist_segment(p_spk, vec2<f32>(-2.0, -3.0), vec2<f32>(2.0, -6.5));
        let d_spk_cone_b = dist_segment(p_spk, vec2<f32>(-2.0, 3.0), vec2<f32>(2.0, 6.5));
        let d_spk_cone_f = dist_segment(p_spk, vec2<f32>(2.0, -6.5), vec2<f32>(2.0, 6.5));
        let in_cone = (p_spk.x >= -2.0) && (p_spk.x <= 2.0) && (abs(p_spk.y) <= 3.0 + (p_spk.x + 2.0) * 0.9);
        let d_cone = select(min(min(d_spk_cone, d_spk_cone_b), d_spk_cone_f), -min(min(d_spk_cone, d_spk_cone_b), d_spk_cone_f), in_cone) - 0.35;
        let mask_speaker = 1.0 - smoothstep(-0.35, 0.35, min(d_spk_box, d_cone));
        
        // Soundwave arc 1 (r = 6.8) and arc 2 (r = 10.8)
        let r_arc = length(p_spk - vec2<f32>(1.0, 0.0));
        let ang_arc = atan2(p_spk.y, p_spk.x - 1.0);
        var mask_wave = 0.0;
        if abs(ang_arc) <= 0.85 {
            let d_arc1 = abs(r_arc - 6.5) - 0.75;
            let d_arc2 = abs(r_arc - 10.5) - 0.75;
            mask_wave = 1.0 - smoothstep(-0.35, 0.35, min(d_arc1, d_arc2));
        }
        let mask_sound = max(mask_speaker, mask_wave) * 0.85;
        
        // ── 8. Adaptive Contrast Colors & Master Composite ──
        let primary_glyph = mix(vec3<f32>(0.96, 0.98, 1.0), vec3<f32>(0.11, 0.14, 0.18), is_light);
        let secondary_glyph = mix(vec3<f32>(0.82, 0.86, 0.90), vec3<f32>(0.28, 0.32, 0.38), is_light);
        let gold = mix(vec3<f32>(0.86, 0.97, 0.26), vec3<f32>(0.14, 0.52, 0.26), is_light);
        let cyan = mix(vec3<f32>(0.38, 0.85, 1.0), vec3<f32>(0.06, 0.44, 0.88), is_light);

        let primary_elems = max(
            max(max(mask_prev, mask_next), max(mask_play_icon, mask_time_text)),
            max(mask_speaker, max(mask_wave, mask_disc_edge))
        );
        let secondary_elems = max(
            max(mask_rail, mask_sep),
            max(mask_disc_fill, max(mask_label, max(grooves, btn_fill * 0.3)))
        );

        if eq_mask > 0.05 {
            app.glyph_mask = max(eq_mask, max(primary_elems, secondary_elems));
            app.glyph_color = mix(gold, primary_glyph, clamp((primary_elems - eq_mask) * 2.0, 0.0, 1.0));
            app.fluid_color = gold;
            app.fluid_glow = exp(-abs(local_p.y + 4.0) * 0.25) * 0.35 * (1.0 - is_light * 0.5);
        } else if mask_fill > 0.05 || mask_knob > 0.05 {
            let active_fill = max(mask_fill, mask_knob);
            app.glyph_mask = max(active_fill, max(primary_elems, secondary_elems));
            app.glyph_color = mix(cyan, primary_glyph, clamp((primary_elems - active_fill) * 2.0, 0.0, 1.0));
            app.fluid_color = cyan;
            app.fluid_glow = exp(-max(d_knob, 0.0) * 0.4) * 0.45 * (1.0 - is_light * 0.5);
        } else if mask_note > 0.05 {
            app.glyph_mask = max(mask_note, max(primary_elems, secondary_elems));
            app.glyph_color = gold;
            app.fluid_color = gold;
            app.fluid_glow = exp(-max(d_note, 0.0) * 0.6) * 0.30 * (1.0 - is_light * 0.5);
        } else if primary_elems > 0.02 {
            app.glyph_mask = primary_elems;
            app.glyph_color = primary_glyph;
        } else {
            app.glyph_mask = secondary_elems;
            app.glyph_color = secondary_glyph;
        }

        // Tactile button glow on hover
        if hover_play > 0.01 {
            let glow_col = mix(vec3<f32>(0.40, 0.85, 1.0), vec3<f32>(0.06, 0.44, 0.88), is_light);
            app.fluid_color = mix(app.fluid_color, glow_col, hover_play);
            app.fluid_glow = max(app.fluid_glow, exp(-max(d_btn, 0.0) * 0.16) * hover_play * 0.35 * (1.0 - is_light * 0.5));
        }
        
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
    // 1. APPLE DUAL-LAYER PHYSICAL DROP SHADOW
    // ════════════════════════════════════════════
    if sdf > 0.0 {
        // Tight contact ambient occlusion directly beneath the perimeter
        let contact = exp(-sdf * 0.50) * 0.28;
        // Diffuse ambient float shadow with gentle downward bias (+6px)
        let p_float = local_px - vec2<f32>(0.0, 6.0);
        let sdf_float = rr_sdf(p_float, half_size, r);
        let diffuse = exp(-max(sdf_float, 0.0) * 0.09) * 0.24;
        let total_shadow = max(contact, diffuse);
        return vec4<f32>(0.0, 0.0, 0.0, total_shadow);
    }
    
    // ════════════════════════════════════════════
    // 2. ANTI-ALIASED SILKY MASK & ANALYTICAL NORMAL
    // ════════════════════════════════════════════
    let mask = 1.0 - smoothstep(-1.0, 0.1, sdf);
    let inside = max(-sdf, 0.0);

    // True Apple VisionOS / macOS Micro-Chamfer (2.5px width)
    let bevel_w = clamp(params.bevel, 1.5, 3.2);
    let t = clamp(inside / bevel_w, 0.0, 1.0); // 0 at outer edge, 1 at interior flat face
    let edge = 1.0 - t; // 1 at outer edge, 0 at interior flat face

    // Exact analytical 2D outward unit normal of the rounded rectangle
    let eps_sdf = 0.5;
    let d_dx = rr_sdf(local_px + vec2<f32>(eps_sdf, 0.0), half_size, r) - rr_sdf(local_px - vec2<f32>(eps_sdf, 0.0), half_size, r);
    let d_dy = rr_sdf(local_px + vec2<f32>(0.0, eps_sdf), half_size, r) - rr_sdf(local_px - vec2<f32>(0.0, eps_sdf), half_size, r);
    let grad_2d = normalize(vec2<f32>(d_dx, d_dy));

    // Continuous 3D surface normal: micro-chamfer bevel profile (planar inside >= 2.5px)
    let theta = edge * 1.5707963;
    let sin_th = sin(theta);
    let cos_th = cos(theta);
    let N = normalize(vec3<f32>(grad_2d * sin_th, cos_th));
    let N_2d = N.xy;

    // ════════════════════════════════════════════
    // 3. PHYSICAL SNELL'S LAW REFRACTION & PRISMATIC CAUSTICS
    // ════════════════════════════════════════════
    let px_to_uv = vec2<f32>(1.0, 1.0) / params.surface_size;

    // Subtle micro-optical deflection strictly at the 2px chamfer (zero interior warp!)
    let refr_dist = 2.0 * params.refraction_strength * sin_th;
    let refr_ray = -grad_2d * refr_dist * px_to_uv;

    // Delicate sub-pixel optical crystal glimmer at the extreme outer edge
    let ca_dist = clamp(params.dispersion * 15.0, 0.0, 0.8) * pow(edge, 3.0);
    let ca_offset = -grad_2d * ca_dist * px_to_uv;

    let uv_r = clamp(uv + refr_ray - ca_offset, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_g = clamp(uv + refr_ray, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_b = clamp(uv + refr_ray + ca_offset, vec2<f32>(0.001), vec2<f32>(0.999));

    // ════════════════════════════════════════════
    // 4. DEEP SILKY FROST + APPLE MATERIAL SUBSTRATE
    // ════════════════════════════════════════════
    // Full interior frosted blur smoothly reaching the chamfer edge
    let frost = smoothstep(0.0, 2.0, inside);
    let local_blur = params.blur_radius * frost;

    // Outer crystal rim sample
    let sharp_r = textureSampleLevel(background_texture, background_sampler, uv_r, 0.0).r;
    let sharp_g = textureSampleLevel(background_texture, background_sampler, uv_g, 0.0).g;
    let sharp_b = textureSampleLevel(background_texture, background_sampler, uv_b, 0.0).b;
    let sharp_col = vec3<f32>(sharp_r, sharp_g, sharp_b);

    var col: vec3<f32>;
    if local_blur <= 0.5 {
        col = sharp_col;
    } else {
        let frosted_col = sample_frost_blur(uv_g, local_blur);
        col = mix(sharp_col, frosted_col, frost);
    }

    // Measure local background luminance for adaptive text contrast
    let bg_luma = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    let is_light = smoothstep(0.55, 0.78, bg_luma);

    // Apple Vibrancy: gently boost saturation so wallpaper colors shine warmly through frosted glass
    let sat_col = mix(vec3<f32>(bg_luma), col, 1.15);
    col = sat_col;

    // True Optical Liquid Glass:
    // Pure, crystal transmission of the real wallpaper underneath with silky frosted blur.
    // Zero muddy tints, zero colored sludge! The glass shows the true background.
    // A delicate 7% pure white silica transmission lift gives physical crystal presence.
    col = mix(col, vec3<f32>(1.0, 1.0, 1.0), 0.07);

    // ════════════════════════════════════════════
    // 5. DIRECTIONAL APPLE SPECULAR & DOUBLE-LINE REFRACTION
    // ════════════════════════════════════════════
    let top_facing = max(-grad_2d.y, 0.0);
    let left_facing = max(-grad_2d.x, 0.0);
    let bot_facing = max(grad_2d.y, 0.0);

    // 1. Outer 1px razor crest specular highlight (Top & Top-Left only)
    let hairline = smoothstep(1.5, 0.1, inside);
    let top_spec = hairline * (pow(top_facing, 1.5) * 0.95 + left_facing * 0.35) * params.specular_strength;

    // 2. Inner 1px chamfer refraction line (Iconic Apple double-line reflection)
    let inner_line = smoothstep(2.6, 1.6, inside) * smoothstep(0.8, 1.8, inside);
    let inner_spec = inner_line * pow(top_facing, 2.0) * 0.40 * params.specular_strength;

    // 3. Bottom edge contact occlusion (grounds the card, NO white bottom stroke!)
    let bot_occlusion = hairline * bot_facing * 0.35;
    col = col * (1.0 - bot_occlusion);

    // 4. Tactile micro-grain for authentic physical glass
    let noise_seed = dot(local_px, vec2<f32>(12.9898, 78.233));
    let micro_grain = (fract(sin(noise_seed) * 43758.5453) - 0.5) * 0.022;

    col += vec3<f32>(top_spec + inner_spec + micro_grain);

    var side_bevel_flare = 0.0;
    var side_rim_flare = 0.0;
    if params.pointer_pos.x >= 0.0 {
        let cur_px = params.pointer_pos * params.surface_size - center;
        let to_cur = cur_px - local_px;
        let dist_to_cur = length(to_cur);
        let dir_to_cur = to_cur / max(dist_to_cur, 0.001);

        // Outward normal of the molded glass edge facing the cursor
        let bevel_facing = max(dot(N_2d, dir_to_cur), 0.0);

        // Dynamic reach: responds as cursor approaches within 100px of the edge
        let prox = clamp(1.0 - (dist_to_cur / 100.0), 0.0, 1.0);
        let prox_curve = prox * prox;

        // Subtle, elegant edge response
        side_bevel_flare = pow(edge, 1.6) * pow(bevel_facing, 3.0) * prox_curve * 0.55 * params.specular_strength;
        side_rim_flare = hairline * pow(bevel_facing, 2.2) * prox * 0.70 * params.specular_strength;
    }

    // ════════════════════════════════════════════
    // 9. CONTROLS, FLUID EMISSION & GLYPHS
    // ════════════════════════════════════════════
    let app = render_element_appearance(local_px, half_size, params.padding2.x, params.padding2.y, params.time, params.tint, is_light);

    // ════════════════════════════════════════════
    // 10. MASTER COMPOSITE
    // ════════════════════════════════════════════
    var fin = col;

    let flare_mag = (side_bevel_flare + side_rim_flare) * mix(1.0, 0.25, is_light);
    let flare_rgb = vec3<f32>(flare_mag * 1.02, flare_mag * 0.99, flare_mag * 1.04);
    fin += flare_rgb;

    fin += app.fluid_color * (app.fluid_glow * mix(1.0, 0.35, is_light));
    fin = mix(fin, app.glyph_color, app.glyph_mask);

    let aa = 1.0 - smoothstep(-1.0, 0.0, sdf);
    return vec4<f32>(fin * aa, aa);
}
