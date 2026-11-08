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

    surface_size: vec2<f32>,
    is_light: f32,
    padding: f32, // element_type (0=base, 1=header, 2=wave, 3=gauge, 4=disc, 5=prev, 6=play, 7=next, 8=slider)
    _pad: vec2<f32>,
}

@group(0) @binding(0) var<uniform> params: GlassUniforms;
@group(1) @binding(0) var background_texture: texture_2d<f32>;
@group(1) @binding(1) var background_sampler: sampler;
@group(1) @binding(2) var overlay_texture: texture_2d<f32>;

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
    
    // Dilation padding in normalized coordinates to prevent edge clipping of anti-aliasing, specular rim and drop shadow
    let quad_pad = vec2<f32>(28.0, 28.0) / params.surface_size;
    let min_bounds = max(params.bounds.xy - quad_pad, vec2<f32>(0.0));
    let max_bounds = min(params.bounds.xy + params.bounds.zw + quad_pad, vec2<f32>(1.0));
    let quad_size = max_bounds - min_bounds;

    let bounds_uv = min_bounds + local_uv * quad_size;
    
    out.uv = bounds_uv;
    out.clip_position = vec4<f32>(bounds_uv.x * 2.0 - 1.0, 1.0 - bounds_uv.y * 2.0, 0.0, 1.0);
    return out;
}

// ── Rounded-rect Signed Distance Field ──
fn rr_sdf(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

fn rr_normal(p: vec2<f32>, b: vec2<f32>, r: f32) -> vec2<f32> {
    let corner_center = b - vec2<f32>(r);
    let ap = abs(p);
    let s = sign(p);
    let q = ap - corner_center;
    if q.x > 0.0 && q.y > 0.0 {
        return s * normalize(q);
    } else if q.x > q.y {
        return vec2<f32>(s.x, 0.0);
    } else {
        return vec2<f32>(0.0, s.y);
    }
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

// ── Apple VisionOS / macOS 32-tap Multi-Scale Golden-Angle Diffusion Blur ──
fn sample_frost_blur(uv: vec2<f32>, radius: f32) -> vec3<f32> {
    if radius <= 0.5 {
        return textureSampleLevel(background_texture, background_sampler, uv, 0.0).rgb;
    }
    let rad_uv = radius / params.surface_size;
    let golden_angle = 2.39996323;

    var col = textureSampleLevel(background_texture, background_sampler, uv, 0.0).rgb * 0.06;
    var total_w = 0.06;

    for (var i = 0.0; i < 32.0; i += 1.0) {
        let t = (i + 0.5) / 32.0;
        let r = sqrt(t);
        let theta = i * golden_angle;
        let offset = vec2<f32>(r * cos(theta), r * sin(theta)) * rad_uv;
        let sample_uv = clamp(uv + offset, vec2<f32>(0.001), vec2<f32>(0.999));
        let s = textureSampleLevel(background_texture, background_sampler, sample_uv, 0.0).rgb;
        let w = exp(-r * r * 1.6);
        col += s * w;
        total_w += w;
    }

    return col / total_w;
}

// ── Struct for Clean Minimalist Element Appearance ──
struct ElementAppearance {
    glyph_mask: f32,
    glyph_color: vec3<f32>,
    fill_mask: f32,
    rim_mask: f32,
    pill_mask: f32,
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

// ── Evaluator for Agenda Schedule Row Typography ──
fn eval_agenda_row_text(p: vec2<f32>, row_idx: u32) -> f32 {
    let row_y = -105.0 + f32(row_idx) * 44.0;
    let tm_start = -135.0;
    let tm_sp = 6.8;
    var d_time = 999.0;
    
    // Time badges: 09:00, 11:30, 14:00, 16:30, 18:00, 20:00
    if row_idx == 0u {
        // 09:00
        let t0 = draw_digit(p - vec2<f32>(tm_start + 0.0 * tm_sp, row_y), 0u, 0.75);
        let t1 = draw_digit(p - vec2<f32>(tm_start + 1.0 * tm_sp, row_y), 9u, 0.75);
        let col = draw_colon(p - vec2<f32>(tm_start + 1.8 * tm_sp, row_y), 0.75);
        let t2 = draw_digit(p - vec2<f32>(tm_start + 2.6 * tm_sp, row_y), 0u, 0.75);
        let t3 = draw_digit(p - vec2<f32>(tm_start + 3.6 * tm_sp, row_y), 0u, 0.75);
        d_time = min(min(min(t0, t1), col), min(t2, t3));
    } else if row_idx == 1u {
        // 11:30
        let t0 = draw_digit(p - vec2<f32>(tm_start + 0.0 * tm_sp, row_y), 1u, 0.75);
        let t1 = draw_digit(p - vec2<f32>(tm_start + 1.0 * tm_sp, row_y), 1u, 0.75);
        let col = draw_colon(p - vec2<f32>(tm_start + 1.8 * tm_sp, row_y), 0.75);
        let t2 = draw_digit(p - vec2<f32>(tm_start + 2.6 * tm_sp, row_y), 3u, 0.75);
        let t3 = draw_digit(p - vec2<f32>(tm_start + 3.6 * tm_sp, row_y), 0u, 0.75);
        d_time = min(min(min(t0, t1), col), min(t2, t3));
    } else if row_idx == 2u {
        // 14:00
        let t0 = draw_digit(p - vec2<f32>(tm_start + 0.0 * tm_sp, row_y), 1u, 0.75);
        let t1 = draw_digit(p - vec2<f32>(tm_start + 1.0 * tm_sp, row_y), 4u, 0.75);
        let col = draw_colon(p - vec2<f32>(tm_start + 1.8 * tm_sp, row_y), 0.75);
        let t2 = draw_digit(p - vec2<f32>(tm_start + 2.6 * tm_sp, row_y), 0u, 0.75);
        let t3 = draw_digit(p - vec2<f32>(tm_start + 3.6 * tm_sp, row_y), 0u, 0.75);
        d_time = min(min(min(t0, t1), col), min(t2, t3));
    } else if row_idx == 3u {
        // 16:30
        let t0 = draw_digit(p - vec2<f32>(tm_start + 0.0 * tm_sp, row_y), 1u, 0.75);
        let t1 = draw_digit(p - vec2<f32>(tm_start + 1.0 * tm_sp, row_y), 6u, 0.75);
        let col = draw_colon(p - vec2<f32>(tm_start + 1.8 * tm_sp, row_y), 0.75);
        let t2 = draw_digit(p - vec2<f32>(tm_start + 2.6 * tm_sp, row_y), 3u, 0.75);
        let t3 = draw_digit(p - vec2<f32>(tm_start + 3.6 * tm_sp, row_y), 0u, 0.75);
        d_time = min(min(min(t0, t1), col), min(t2, t3));
    } else if row_idx == 4u {
        // 18:00
        let t0 = draw_digit(p - vec2<f32>(tm_start + 0.0 * tm_sp, row_y), 1u, 0.75);
        let t1 = draw_digit(p - vec2<f32>(tm_start + 1.0 * tm_sp, row_y), 8u, 0.75);
        let col = draw_colon(p - vec2<f32>(tm_start + 1.8 * tm_sp, row_y), 0.75);
        let t2 = draw_digit(p - vec2<f32>(tm_start + 2.6 * tm_sp, row_y), 0u, 0.75);
        let t3 = draw_digit(p - vec2<f32>(tm_start + 3.6 * tm_sp, row_y), 0u, 0.75);
        d_time = min(min(min(t0, t1), col), min(t2, t3));
    } else if row_idx == 5u {
        // 20:00
        let t0 = draw_digit(p - vec2<f32>(tm_start + 0.0 * tm_sp, row_y), 2u, 0.75);
        let t1 = draw_digit(p - vec2<f32>(tm_start + 1.0 * tm_sp, row_y), 0u, 0.75);
        let col = draw_colon(p - vec2<f32>(tm_start + 1.8 * tm_sp, row_y), 0.75);
        let t2 = draw_digit(p - vec2<f32>(tm_start + 2.6 * tm_sp, row_y), 0u, 0.75);
        let t3 = draw_digit(p - vec2<f32>(tm_start + 3.6 * tm_sp, row_y), 0u, 0.75);
        d_time = min(min(min(t0, t1), col), min(t2, t3));
    }

    let ev_start = -92.0;
    let ev_sp = 8.0;
    let sc = 0.95;
    var d_title = 999.0;
    if row_idx == 0u {
        // STANDUP
        let c0 = draw_clock_letter(p - vec2<f32>(ev_start + 0.0 * ev_sp, row_y), 83u, sc);
        let c1 = draw_clock_letter(p - vec2<f32>(ev_start + 1.0 * ev_sp, row_y), 84u, sc);
        let c2 = draw_clock_letter(p - vec2<f32>(ev_start + 2.0 * ev_sp, row_y), 65u, sc);
        let c3 = draw_clock_letter(p - vec2<f32>(ev_start + 3.0 * ev_sp, row_y), 78u, sc);
        let c4 = draw_clock_letter(p - vec2<f32>(ev_start + 4.0 * ev_sp, row_y), 68u, sc);
        let c5 = draw_clock_letter(p - vec2<f32>(ev_start + 5.0 * ev_sp, row_y), 85u, sc);
        let c6 = draw_clock_letter(p - vec2<f32>(ev_start + 6.0 * ev_sp, row_y), 80u, sc);
        d_title = min(min(min(c0, c1), min(c2, c3)), min(min(c4, c5), c6));
    } else if row_idx == 1u {
        // CODE REV
        let c0 = draw_clock_letter(p - vec2<f32>(ev_start + 0.0 * ev_sp, row_y), 67u, sc);
        let c1 = draw_clock_letter(p - vec2<f32>(ev_start + 1.0 * ev_sp, row_y), 79u, sc);
        let c2 = draw_clock_letter(p - vec2<f32>(ev_start + 2.0 * ev_sp, row_y), 68u, sc);
        let c3 = draw_clock_letter(p - vec2<f32>(ev_start + 3.0 * ev_sp, row_y), 69u, sc);
        let c4 = draw_clock_letter(p - vec2<f32>(ev_start + 4.6 * ev_sp, row_y), 82u, sc);
        let c5 = draw_clock_letter(p - vec2<f32>(ev_start + 5.6 * ev_sp, row_y), 69u, sc);
        let c6 = draw_clock_letter(p - vec2<f32>(ev_start + 6.6 * ev_sp, row_y), 86u, sc);
        d_title = min(min(min(c0, c1), min(c2, c3)), min(min(c4, c5), c6));
    } else if row_idx == 2u {
        // DEEP WORK
        let c0 = draw_clock_letter(p - vec2<f32>(ev_start + 0.0 * ev_sp, row_y), 68u, sc);
        let c1 = draw_clock_letter(p - vec2<f32>(ev_start + 1.0 * ev_sp, row_y), 69u, sc);
        let c2 = draw_clock_letter(p - vec2<f32>(ev_start + 2.0 * ev_sp, row_y), 69u, sc);
        let c3 = draw_clock_letter(p - vec2<f32>(ev_start + 3.0 * ev_sp, row_y), 80u, sc);
        let c4 = draw_clock_letter(p - vec2<f32>(ev_start + 4.6 * ev_sp, row_y), 87u, sc);
        let c5 = draw_clock_letter(p - vec2<f32>(ev_start + 5.6 * ev_sp, row_y), 79u, sc);
        let c6 = draw_clock_letter(p - vec2<f32>(ev_start + 6.6 * ev_sp, row_y), 82u, sc);
        let c7 = draw_clock_letter(p - vec2<f32>(ev_start + 7.6 * ev_sp, row_y), 75u, sc);
        d_title = min(min(min(c0, c1), min(c2, c3)), min(min(c4, c5), min(c6, c7)));
    } else if row_idx == 3u {
        // PLANNING
        let c0 = draw_clock_letter(p - vec2<f32>(ev_start + 0.0 * ev_sp, row_y), 80u, sc);
        let c1 = draw_clock_letter(p - vec2<f32>(ev_start + 1.0 * ev_sp, row_y), 76u, sc);
        let c2 = draw_clock_letter(p - vec2<f32>(ev_start + 2.0 * ev_sp, row_y), 65u, sc);
        let c3 = draw_clock_letter(p - vec2<f32>(ev_start + 3.0 * ev_sp, row_y), 78u, sc);
        let c4 = draw_clock_letter(p - vec2<f32>(ev_start + 4.0 * ev_sp, row_y), 78u, sc);
        let c5 = draw_clock_letter(p - vec2<f32>(ev_start + 5.0 * ev_sp, row_y), 73u, sc);
        let c6 = draw_clock_letter(p - vec2<f32>(ev_start + 6.0 * ev_sp, row_y), 78u, sc);
        let c7 = draw_clock_letter(p - vec2<f32>(ev_start + 7.0 * ev_sp, row_y), 71u, sc);
        d_title = min(min(min(c0, c1), min(c2, c3)), min(min(c4, c5), min(c6, c7)));
    } else if row_idx == 4u {
        // WRAP UP
        let c0 = draw_clock_letter(p - vec2<f32>(ev_start + 0.0 * ev_sp, row_y), 87u, sc);
        let c1 = draw_clock_letter(p - vec2<f32>(ev_start + 1.0 * ev_sp, row_y), 82u, sc);
        let c2 = draw_clock_letter(p - vec2<f32>(ev_start + 2.0 * ev_sp, row_y), 65u, sc);
        let c3 = draw_clock_letter(p - vec2<f32>(ev_start + 3.0 * ev_sp, row_y), 80u, sc);
        let c4 = draw_clock_letter(p - vec2<f32>(ev_start + 4.6 * ev_sp, row_y), 85u, sc);
        let c5 = draw_clock_letter(p - vec2<f32>(ev_start + 5.6 * ev_sp, row_y), 80u, sc);
        d_title = min(min(min(c0, c1), min(c2, c3)), min(c4, c5));
    } else if row_idx == 5u {
        // WORKOUT
        let c0 = draw_clock_letter(p - vec2<f32>(ev_start + 0.0 * ev_sp, row_y), 87u, sc);
        let c1 = draw_clock_letter(p - vec2<f32>(ev_start + 1.0 * ev_sp, row_y), 79u, sc);
        let c2 = draw_clock_letter(p - vec2<f32>(ev_start + 2.0 * ev_sp, row_y), 82u, sc);
        let c3 = draw_clock_letter(p - vec2<f32>(ev_start + 3.0 * ev_sp, row_y), 75u, sc);
        let c4 = draw_clock_letter(p - vec2<f32>(ev_start + 4.0 * ev_sp, row_y), 79u, sc);
        let c5 = draw_clock_letter(p - vec2<f32>(ev_start + 5.0 * ev_sp, row_y), 85u, sc);
        let c6 = draw_clock_letter(p - vec2<f32>(ev_start + 6.0 * ev_sp, row_y), 84u, sc);
        d_title = min(min(min(c0, c1), min(c2, c3)), min(min(c4, c5), c6));
    }

    return min(d_time, d_title);
}

// ── Serene Liquid Glass Clock Appearance ──
fn render_element_appearance(local_p: vec2<f32>, half: vec2<f32>, el_type: f32, progress: f32, time: f32, tint: vec4<f32>, is_light: f32) -> ElementAppearance {
    var app: ElementAppearance;
    app.glyph_mask = 0.0;
    app.glyph_color = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.11, 0.14, 0.18), is_light);
    app.fill_mask = 0.0;
    app.rim_mask = 0.0;
    app.pill_mask = 0.0;
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
        // Pure Liquid Glass Clock Canvas: All typography, calendar icon, and battery capsule
        // are rendered via the subpixel-perfect UIFont vector overlay.
        return app;
    }

    // ─────────────────────────────────────────────────────────────
    // 26. Apple Liquid Glass Todo & Calendar Widget (360x455)
    // ─────────────────────────────────────────────────────────────
    if el_type >= 25.5 && el_type <= 26.5 {
        // Pure Liquid Glass Todo & Calendar Canvas:
        // Text, icons, checkboxes, and interactive sliding tabs are rendered
        // via the subpixel-perfect UIFont vector overlay.
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
        let mask_disc_edge = smoothstep(0.3, -0.3, abs(d_disc) - 0.45);
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
        
        // ── 3. Subtle Vertical Crystalline Divider (x = -26.0, y = -10.0, h = 14.0, terminates 6px above timeline) ──
        let d_sep = rr_sdf(local_p - vec2<f32>(-26.0, -10.0), vec2<f32>(0.5, 14.0), 0.5);
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
        // Total track spans from x = 8.0 to x = 138.0 (width = 130.0)
        let bar_len = 130.0;
        let d_rail = rr_sdf(local_p - vec2<f32>(73.0, y_bar), vec2<f32>(65.0, 1.8), 1.4);
        let mask_rail = (1.0 - smoothstep(-0.35, 0.35, d_rail)) * 0.28;
        
        let fill_w = prog * bar_len;
        let fill_cx = 8.0 + fill_w * 0.5;
        let d_fill = rr_sdf(local_p - vec2<f32>(fill_cx, y_bar), vec2<f32>(max(fill_w * 0.5, 1.0), 2.0), 1.4);
        let mask_fill = 1.0 - smoothstep(-0.35, 0.35, d_fill);
        
        // Scrubber Knob (circle r = 4.2)
        let p_knob = local_p - vec2<f32>(8.0 + fill_w, y_bar);
        let d_knob = length(p_knob) - 4.5;
        let mask_knob = 1.0 - smoothstep(-0.35, 0.35, d_knob);
        
        // ── 6. Elapsed Time Display (x = -24.0, y = 22.0) ──
        let cur_sec = u32(elapsed_sec);
        let e_min = (cur_sec / 60u) % 10u;
        let e_s_tens = (cur_sec % 60u) / 10u;
        let e_s_ones = cur_sec % 10u;
        let t_scale = 1.05;
        let tsp = 7.2;
        let d_em = draw_digit(local_p - vec2<f32>(-28.0 - tsp, y_bar), e_min, t_scale);
        let d_ecol = draw_colon(local_p - vec2<f32>(-28.0, y_bar), t_scale);
        let d_es0 = draw_digit(local_p - vec2<f32>(-28.0 + tsp * 0.9, y_bar), e_s_tens, t_scale);
        let d_es1 = draw_digit(local_p - vec2<f32>(-28.0 + tsp * 1.9, y_bar), e_s_ones, t_scale);
        let d_time_text = min(min(d_em, d_ecol), min(d_es0, d_es1)) - 0.42;
        let mask_time_text = (1.0 - smoothstep(-0.35, 0.35, d_time_text)) * 0.85;
        
        // ── 7. Volume Speaker Icon & Soundwaves (x = 148.0, y = y_ctrl) ──
        let p_spk = local_p - vec2<f32>(148.0, y_ctrl);
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
    
    // ════════════════════════════════════════════
    // CLEAN GEOMETRIC CONTOUR (ZERO SQUARE SHADOW BOX)
    // ════════════════════════════════════════════
    // Convert surface-space UV (from vertex shader) to local bounds-relative coordinates
    let local_uv = vec2<f32>(
        (uv.x - params.bounds.x) / params.bounds.z,
        (uv.y - params.bounds.y) / params.bounds.w
    );
    
    let bounds_px = vec4<f32>(
        params.bounds.x * params.surface_size.x,
        params.bounds.y * params.surface_size.y,
        params.bounds.z * params.surface_size.x,
        params.bounds.w * params.surface_size.y
    );
    let half_size = vec2<f32>(bounds_px.z * 0.5, bounds_px.w * 0.5);
    let center = vec2<f32>(bounds_px.x + half_size.x, bounds_px.y + half_size.y);
    let local_px = uv * params.surface_size - center;
    let r = min(params.corner_radius, min(half_size.x, half_size.y));
    
    // Crisp, stable rounded-rectangle SDF
    let sdf = rr_sdf(local_px, half_size, r);

    let style_mode = params._pad.x; // 0.0 = Obsidian (Clean), 1.0 = Glassmorphism (Frosted), 2.0 = Liquid Glass
    let is_light = params.is_light;
    
    // Outside the rounded rectangle: soft ambient drop shadow or transparent
    if sdf > 0.0 {
        if style_mode >= 0.5 && sdf <= 26.0 {
            let bg_under_shadow = textureSampleLevel(background_texture, background_sampler, uv, 0.0).rgb;
            let shadow_luma = dot(bg_under_shadow, vec3<f32>(0.2126, 0.7152, 0.0722));
            let is_shadow_light = (is_light > 0.5 || shadow_luma > 0.50);

            // Dual-lobe shadow: tight contact occlusion (0-6px) + soft ambient spread (6-26px)
            let contact = exp(-sdf * 0.35) * 0.40;
            // On light backgrounds, ambient shadow is deeper and wider to lift the card off the surface
            let ambient_weight = select(0.18, 0.42, is_shadow_light);
            let ambient = exp(-sdf * 0.08) * ambient_weight;
            let shadow_alpha = (contact + ambient) * (1.0 - smoothstep(20.0, 26.0, sdf));
            return vec4<f32>(vec3<f32>(0.0, 0.0, 0.0), shadow_alpha);
        }
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    
    // ════════════════════════════════════════════
    // 2. ANALYTIC CONVEX SQUIRCLE BEZEL & 2D OUTWARD NORMAL
    // ════════════════════════════════════════════
    let depth = max(-sdf, 0.0);
    let bezel = clamp(params.bevel, 10.0, 36.0); // 10-36px physical bezel
    let t = clamp(depth / bezel, 0.0, 1.0);       // 0.0 at outer edge, 1.0 where bezel ends

    // Exact analytical 2D outward unit normal of rounded rectangle
    let grad_2d = rr_normal(local_px, half_size, r);
    let corner_factor = clamp(abs(grad_2d.x * grad_2d.y) * 4.0, 0.0, 1.0);

    // ════════════════════════════════════════════
    // 3. PHYSICAL 3D SNELL'S LAW REFRACTION BAND & CAUCHY DISPERSION
    // ════════════════════════════════════════════
    // Optical refraction vector combines perimeter normal with radial center vector.
    // 0.65 radial mix delivers decisive tangential deflection across straight horizontal and vertical grid lines!
    let radial_dir = normalize(local_px);
    let refract_dir = normalize(mix(grad_2d, radial_dir, 0.65));

    // Superellipse height derivative h'(t) with high slope clamp
    let u = clamp(1.0 - t, 0.0, 1.0);
    let u4 = u * u * u * u;
    let denom = pow(max(1.0 - u4, 0.0001), 0.75);
    let raw_slope = (u * u * u) / denom;
    let se_slope = clamp(raw_slope, 0.0, 6.0) / 6.0;

    // Smooth cosine spread across full bezel depth (ensures wide 14-22px visible optical bend)
    let wide_spread = cos(t * 1.5707963);

    // Blended gentler profile: maintains high rim tilt while staying elevated across the entire bezel
    let lens_profile = mix(wide_spread, se_slope, 0.35);

    let thickness = select(24.0, params.refraction_strength, params.refraction_strength >= 1.0);
    let off_px = refract_dir * (thickness * lens_profile);
    let off_uv = off_px / params.surface_size;

    let debug_mode = params._pad.y;
    if debug_mode > 0.5 {
        let mag = length(off_px) / thickness;
        let r_disp = abs(off_px.x) / thickness;
        let b_disp = abs(off_px.y) / thickness;
        return vec4<f32>(r_disp, mag, b_disp, 1.0);
    }

    // Chromatic Cauchy dispersion (scaled by params.dispersion, default 0.28):
    // Separation is 4-6px bands across the wide bezel!
    let disp = select(0.28, params.dispersion, params.dispersion >= 0.05);
    let uv_r = clamp(uv + off_uv * (1.0 - disp), vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_g = clamp(uv + off_uv * 1.00, vec2<f32>(0.001), vec2<f32>(0.999));
    let uv_b = clamp(uv + off_uv * (1.0 + disp), vec2<f32>(0.001), vec2<f32>(0.999));

    let sharp_r = textureSampleLevel(background_texture, background_sampler, uv_r, 0.0).r;
    let sharp_g = textureSampleLevel(background_texture, background_sampler, uv_g, 0.0).g;
    let sharp_b = textureSampleLevel(background_texture, background_sampler, uv_b, 0.0).b;
    let sharp_col = vec3<f32>(sharp_r, sharp_g, sharp_b);

    // Dynamic local background luminance adaptation:
    // Respects the surface is_light uniform, while seamlessly responding to local background luminance
    let local_luma = dot(sharp_col, vec3<f32>(0.2126, 0.7152, 0.0722));
    let effective_light = select(select(0.0, 1.0, local_luma > 0.55), 1.0, is_light > 0.5);

    var col: vec3<f32>;

    if style_mode >= 1.5 {
        // ── 2.0: LIQUID GLASS ──
        // For notifications (el_type == 27.0), blend subtle blur for crisp text contrast
        let is_island = (params.padding >= 26.9 && params.padding <= 27.1);
        let base_blur = select(0.0, 4.0, is_island);
        let cur_radius = max(base_blur, 2.0 * t);
        let frosted_col = sample_frost_blur(uv, cur_radius);
        let base_crystal = mix(sharp_col, frosted_col, select(0.20 * t, 0.45, is_island));

        let local_crystal_luma = dot(base_crystal, vec3<f32>(0.2126, 0.7152, 0.0722));

        if effective_light > 0.5 {
            // Pristine optical glass over light backdrops (linear light space)
            let light_density = mix(0.92, 0.86, (local_px.y / half_size.y) * 0.5 + 0.5);
            col = base_crystal * light_density;
        } else {
            // Luminous smoked crystal over dark backdrops:
            // 0.003 linear ambient sheen gives delicate ~15/255 sRGB presence (never gray mud!)
            let top_sheen = mix(0.003, 0.0005, (local_px.y / half_size.y) * 0.5 + 0.5);
            col = base_crystal * 1.04 + vec3<f32>(top_sheen) * clamp(1.0 - local_crystal_luma * 4.0, 0.0, 1.0);
        }

        // Faint diagonal gradient across interior (top-left lighter 3-5%) for physical glass depth
        let diag_t = clamp((local_px.x + local_px.y) / (half_size.x + half_size.y) * 0.5 + 0.5, 0.0, 1.0);
        col *= mix(1.03, 0.97, diag_t);
    } else if style_mode >= 0.5 {
        // ── 1.0: LUXURIOUS APPLE / VISIONOS FROSTED GLASSMORPHISM ──
        // Deep 32-tap multi-scale backdrop diffusion (24px radius)
        let frosted_col = sample_frost_blur(uv, 24.0);
        
        // 1. Color Vibrancy & Saturation Boost (Apple Vibrancy algorithm in linear space)
        let f_luma = dot(frosted_col, vec3<f32>(0.2126, 0.7152, 0.0722));
        let vibrant_col = max(mix(vec3<f32>(f_luma), frosted_col, 1.35), vec3<f32>(0.0));

        // 2. Physical Glass Satin Sheen & Translucency
        if effective_light > 0.5 {
            // Apple Porcelain Ceramic Glass (Light / White mode)
            let light_density = mix(0.90, 0.84, (local_px.y / half_size.y) * 0.5 + 0.5);
            let ceramic_base = mix(vibrant_col * light_density, vec3<f32>(0.92, 0.94, 0.98), 0.18);
            col = ceramic_base;
        } else {
            // Apple Smoked Acrylic Satin Glass (Dark / Vibrant mode)
            // Vibrant colors transmit cleanly, topped with delicate directional ambient sheen
            let top_ambient = mix(0.006, 0.001, (local_px.y / half_size.y) * 0.5 + 0.5);
            col = (vibrant_col * 1.05) + vec3<f32>(top_ambient * clamp(1.0 - f_luma * 4.0, 0.0, 1.0));
        }
    } else {
        // ── 0.0: OBSIDIAN / CLEAN BACKGROUND ──
        col = sharp_col;
    }

    // ════════════════════════════════════════════
    // 4. DIRECTIONAL SPECULAR HAIRLINE & INNER HIGHLIGHT
    // ════════════════════════════════════════════
    let light_vec_2d = normalize(vec2<f32>(-0.45, -0.65)); // Top-left light source
    let light_facing = max(dot(grad_2d, light_vec_2d), 0.0);

    // Directional weight: ~100% at top-left and ~30% at bottom-right (gives glass tangible thickness)
    let dir_weight = mix(0.30, 1.00, light_facing);

    // Crisp 1.0 - 1.5px specular line right at the outer edge
    let corner_mult = 1.0 + 0.35 * corner_factor;
    let rim_1px = smoothstep(1.5, 0.2, depth) * corner_mult;

    // Soft 2.0 - 4.0px inner highlight
    let inner_highlight = smoothstep(4.0, 0.8, depth) * 0.20;

    let spec_rim = (rim_1px * dir_weight + inner_highlight * dir_weight) * params.specular_strength;

    // ════════════════════════════════════════════
    // 5. MASTER COMPOSITE (OBSIDIAN, GLASSMORPHISM, LIQUID GLASS)
    // ════════════════════════════════════════════
    var base_surface: vec3<f32>;

    if style_mode < 0.5 {
        // ── 0.0: SOLID CLEAN BACKGROUND (Pure White on light, Pure Black on dark) ──
        if effective_light > 0.5 {
            let edge_subtle = smoothstep(1.5, 0.0, depth) * 0.06;
            base_surface = vec3<f32>(1.0, 1.0, 1.0) - vec3<f32>(edge_subtle);
        } else {
            base_surface = vec3<f32>(0.0, 0.0, 0.0);
        }
    } else if effective_light > 0.5 {
        // ── LIGHT / WHITE BACKDROP: PHYSICAL DARK-FIELD MENISCUS OCCLUSION ──
        // On white, light is refracted away at the curved rim, creating a crisp 1.5px dark outline
        // that defines the physical glass contour against white surfaces!
        let edge_contour = smoothstep(1.8, 0.0, depth) * 0.16;
        let inner_bevel = smoothstep(4.0, 1.2, depth) * 0.03 * dir_weight;
        base_surface = (col - vec3<f32>(edge_contour)) + vec3<f32>(inner_bevel);
    } else {
        // ── DARK / VIBRANT BACKDROP: DIAMOND-CUT SPECULAR HAIRLINE ──
        let rim_reflection = (rim_1px * 0.95 + inner_highlight * 0.40) * dir_weight;
        let rim_tint = vec3<f32>(1.0, 1.0, 1.0);
        base_surface = col + rim_tint * rim_reflection;
    }

    let app = render_element_appearance(local_px, half_size, params.padding, params.tint.y, params.time, params.tint, effective_light);
    let side_bevel_flare = 0.0;
    let side_rim_flare = 0.0;

    var fin = base_surface;

    let flare_mag = (side_bevel_flare + side_rim_flare) * mix(1.0, 0.25, effective_light);
    let flare_rgb = vec3<f32>(flare_mag * 1.02, flare_mag * 0.99, flare_mag * 1.04);
    fin += flare_rgb;

    fin += app.fluid_color * (app.fluid_glow * mix(1.0, 0.35, effective_light));
    
    // Dedicated high-contrast selected pill fill (85-90% opacity pure white / light ceramic)
    let pill_fill_target = vec3<f32>(0.96, 0.97, 1.0);
    fin = mix(fin, pill_fill_target, app.pill_mask * 0.88);

    // Low-alpha neutral fill and hairline: white lift on dark, black dip on light (preserves pure backdrop hue)
    let fill_target = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.0, 0.0, 0.0), effective_light);
    let fill_strength = mix(0.12, 0.06, effective_light);
    fin = mix(fin, fill_target, app.fill_mask * fill_strength);

    // 1px subtle hairline rim highlight
    let rim_target = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.45, 0.48, 0.55), effective_light);
    fin = mix(fin, rim_target, app.rim_mask * mix(0.20, 0.14, effective_light));

    fin = mix(fin, app.glyph_color, app.glyph_mask);

    // Composite dynamic interactive UI vector overlay
    let overlay = textureSampleLevel(overlay_texture, background_sampler, uv, 0.0);
    fin = mix(fin, overlay.rgb, overlay.a);

    var fin_a = 1.0;
    if params.padding >= 26.9 && params.padding <= 27.1 {
        // Dynamic Island: smooth spring alpha fade during toggle/hide
        fin_a = clamp(params.tint.z, 0.0, 1.0);
    }

    let aa = 1.0 - smoothstep(-1.0, 0.0, sdf);
    return vec4<f32>(fin * aa, fin_a * aa);
}
