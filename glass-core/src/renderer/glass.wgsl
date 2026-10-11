struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    
    // Create a quad from two triangles based on vertex index (0 to 5)
    // 0: top-left, 1: bottom-left, 2: top-right
    // 3: top-right, 4: bottom-left, 5: bottom-right
    
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
    
    // Add 10% padding to bounds for blur/shadows
    let pad = 0.1;
    let padded_x = params.bounds.x - params.bounds.z * pad;
    let padded_y = params.bounds.y - params.bounds.w * pad;
    let padded_w = params.bounds.z * (1.0 + pad * 2.0);
    let padded_h = params.bounds.w * (1.0 + pad * 2.0);
    
    let screen_x = padded_x + local_uv.x * padded_w;
    let screen_y = padded_y + local_uv.y * padded_h;
    
    out.uv = vec2<f32>(screen_x, screen_y); // Global UV for background sampling
    
    // Map to clip space (-1 to 1)
    let clip_x = screen_x * 2.0 - 1.0;
    let clip_y = 1.0 - screen_y * 2.0; // Y is flipped in clip space
    
    out.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    return out;
}

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

// SDF for rounded rect
fn sd_round_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// Deformation function
fn deform(p: vec2<f32>, center: vec2<f32>) -> vec2<f32> {
    var dp = vec2<f32>(0.0);
    
    // Ambient liquid animation - gentle continuous organic sway
    if params.deformation > 0.0 {
        let wave1 = sin(p.y * 0.015 + params.time * 1.8);
        let wave2 = cos(p.x * 0.015 + params.time * 1.4);
        dp.x += wave1 * params.deformation * 16.0;
        dp.y += wave2 * params.deformation * 16.0;
    }
    
    // Interactive cursor ripple & viscous stretch
    let world_p = p + center;
    let pointer_pos_px = params.pointer_pos * params.surface_size;
    let dist_to_pointer = distance(world_p, pointer_pos_px);
    let pointer_effect_radius = 260.0;
    
    if dist_to_pointer < pointer_effect_radius && length(params.pointer_velocity) > 0.0 {
        let t = 1.0 - (dist_to_pointer / pointer_effect_radius);
        let t2 = t * t;
        let ripple = sin(dist_to_pointer * 0.08 - params.time * 16.0) * exp(-dist_to_pointer * 0.018);
        let vel = params.pointer_velocity * 40.0;
        let vel_mag = clamp(length(vel), 0.0, 1.0);
        
        // Fluid advection stretch along pointer velocity
        let advection = vel * t2 * 22.0;
        // Radial ripple displacement
        let dir = normalize(world_p - pointer_pos_px);
        let radial = dir * ripple * 18.0 * vel_mag * t;
        
        dp += advection + radial;
    }
    
    return p + dp;
}

// Calculate true physical surface normal from SDF, bevel profile, curvature dome, and liquid displacement
fn get_normal(p: vec2<f32>, center: vec2<f32>, b: vec2<f32>, r: f32) -> vec3<f32> {
    let eps = 1.0; // 1 pixel delta for numerical gradient
    let p_def = deform(p, center);
    let d0 = sd_round_box(p_def, b, r);
    
    let p_x = deform(p + vec2<f32>(eps, 0.0), center);
    let dx = sd_round_box(p_x, b, r) - d0;
    
    let p_y = deform(p + vec2<f32>(0.0, eps), center);
    let dy = sd_round_box(p_y, b, r) - d0;
    
    let grad = normalize(vec2<f32>(dx, dy) + vec2<f32>(1e-5, 1e-5));
    
    // 1. Edge Bevel Curve: Quartic ease-out profile creating a smooth optical meniscus
    let bevel_w = max(params.bevel, 16.0);
    let edge_dist = clamp(-d0, 0.0, bevel_w);
    let bevel_t = 1.0 - (edge_dist / bevel_w);
    let bevel_slope = pow(bevel_t, 2.5); // Steepest at outer rim, gently flattening inward
    
    // 2. Optical Dome Curvature: Convex lens body across the entire volume
    let norm_p = clamp(p / max(b, vec2<f32>(1.0, 1.0)), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0));
    let dome_grad = norm_p * clamp(params.curvature, 0.0, 1.5);
    
    // 3. Dynamic Wave Slopes from liquid perturbation
    let wave_slope_x = (p_def.x - p.x) * 0.08;
    let wave_slope_y = (p_def.y - p.y) * 0.08;
    
    let nx = grad.x * bevel_slope * 1.8 + dome_grad.x * 0.45 + wave_slope_x;
    let ny = grad.y * bevel_slope * 1.8 + dome_grad.y * 0.45 + wave_slope_y;
    let nz = 1.0;
    
    return normalize(vec3<f32>(nx, ny, nz));
}

// 64-tap Vogel Golden Spiral filter for artifact-free creamy blur
fn blurred_sample(uv: vec2<f32>, radius: f32) -> vec3<f32> {
    if (radius <= 0.0) {
        return textureSampleLevel(background_texture, background_sampler, uv, 0.0).rgb;
    }
    
    var color = vec3<f32>(0.0);
    let samples = 64.0;
    let golden_angle = 2.39996323;
    
    for (var i = 0.0; i < samples; i += 1.0) {
        let r = sqrt(i + 0.5) / sqrt(samples);
        let theta = i * golden_angle;
        let offset = vec2<f32>(r * cos(theta), r * sin(theta)) * radius;
        let sample_uv = clamp(uv + offset / params.surface_size, vec2<f32>(0.001, 0.001), vec2<f32>(0.999, 0.999));
        color += textureSampleLevel(background_texture, background_sampler, sample_uv, 0.0).rgb;
    }
    
    return color / samples;
}

// macOS Color Vibrancy - Saturation boost, contrast curve, and subtle luminant lift
fn apply_vibrancy(rgb: vec3<f32>, saturation_boost: f32) -> vec3<f32> {
    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    var saturated = mix(vec3<f32>(luma), rgb, saturation_boost);
    // Subtle S-curve contrast enhancement
    saturated = saturated * saturated * (vec3<f32>(3.0) - vec3<f32>(2.0) * saturated);
    // Slight gamma correction to keep midtones lively
    return pow(max(saturated, vec3<f32>(0.0)), vec3<f32>(0.92));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    let center = vec2<f32>(params.bounds.x + params.bounds.z * 0.5, params.bounds.y + params.bounds.w * 0.5);
    let size = vec2<f32>(params.bounds.z * 0.5, params.bounds.w * 0.5) * params.surface_size;
    let radius = params.corner_radius; 
    
    // Pixel-space evaluation for isotropic curvature
    let local_p = (uv - center) * params.surface_size;
    let def_p = deform(local_p, center * params.surface_size);
    let dist = sd_round_box(def_p, size, radius);
    
    // Anti-aliased outer boundary
    let alpha = 1.0 - smoothstep(0.0, 1.5, dist);
    if (alpha <= 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    
    let normal = get_normal(local_p, center * params.surface_size, size, radius);
    let view_dir = vec3<f32>(0.0, 0.0, -1.0);
    
    // Snell's Law Refraction Ray
    let eta = 1.0 / params.ior; 
    let refr_dir = refract(view_dir, normal, eta);
    
    // Optical thickness model (thicker in center, tapering at edges)
    let norm_radius = clamp(length(local_p / max(size, vec2<f32>(1.0, 1.0))), 0.0, 1.0);
    let glass_thickness = sqrt(max(0.0, 1.0 - norm_radius * norm_radius)) * 0.5 + 0.5;
    
    // True Non-linear Optical Lens Distortion (Barrel / Magnification)
    let refr_magnitude = params.refraction_strength * 0.12 * glass_thickness;
    let refr_offset = refr_dir.xy * refr_magnitude;
    let disp = params.dispersion * 0.015;
    let blur = params.blur_radius;
    
    // Sample with chromatic dispersion
    let uv_r = clamp(uv + refr_offset * (1.0 + disp), vec2<f32>(0.001, 0.001), vec2<f32>(0.999, 0.999));
    let uv_g = clamp(uv + refr_offset, vec2<f32>(0.001, 0.001), vec2<f32>(0.999, 0.999));
    let uv_b = clamp(uv + refr_offset * (1.0 - disp), vec2<f32>(0.001, 0.001), vec2<f32>(0.999, 0.999));
    
    let r_col = blurred_sample(uv_r, blur).r;
    let g_col = blurred_sample(uv_g, blur).g;
    let b_col = blurred_sample(uv_b, blur).b;
    var base_color = vec3<f32>(r_col, g_col, b_col);
    
    // Apply macOS-grade vibrancy boost (1.35x saturation + contrast calibration)
    var vibrant_color = apply_vibrancy(base_color, 1.35);
    
    // Material tint & subtle luminant tone
    if (params.tint.a > 0.0) {
        vibrant_color = mix(vibrant_color, params.tint.rgb, params.tint.a);
    }
    
    // Physically based Fresnel reflectance (Schlick's approximation)
    let f0 = 0.045; // Optical glass baseline reflectance
    let cos_theta = max(dot(view_dir, -normal), 0.0);
    let fresnel = f0 + (1.0 - f0) * pow(1.0 - cos_theta, 4.0);
    
    // Dual Light Specular Highlights (Key Light + Ambient Rim Light)
    let key_light = normalize(vec3<f32>(0.75, -1.0, -1.2));
    let half_key = normalize(key_light + view_dir);
    let spec_key = pow(max(dot(normal, half_key), 0.0), 96.0) * params.specular_strength * 1.2;
    
    let rim_light = normalize(vec3<f32>(-0.6, 0.8, -0.9));
    let half_rim = normalize(rim_light + view_dir);
    let spec_rim = pow(max(dot(normal, half_rim), 0.0), 48.0) * params.specular_strength * 0.45;
    
    // Composite Glass Illuminations
    var final_color = vibrant_color;
    final_color += vec3<f32>(0.95, 0.98, 1.0) * fresnel * params.fresnel_strength * 0.85;
    final_color += vec3<f32>(1.0, 1.0, 1.0) * (spec_key + spec_rim);
    
    // Optical Bevel Edge Lighting
    // Meniscus highlight catches ambient light at the convex bevel
    let edge_dist_px = -dist;
    let bevel_w_px = max(params.bevel, 16.0);
    if (edge_dist_px >= 0.0 && edge_dist_px < bevel_w_px) {
        let edge_norm = edge_dist_px / bevel_w_px;
        let rim_intensity = pow(1.0 - edge_norm, 3.0);
        let light_dir_2d = normalize(vec2<f32>(0.7, -0.7));
        let rim_alignment = max(dot(normal.xy, light_dir_2d), 0.0);
        
        // Crisp specular line along the top/left crest
        final_color += vec3<f32>(1.0, 1.0, 1.0) * rim_intensity * (0.35 + 0.45 * rim_alignment);
        
        // Subtle interior ambient shadow along bottom/right bevel base
        let shadow_alignment = max(dot(normal.xy, -light_dir_2d), 0.0);
        final_color *= (1.0 - rim_intensity * shadow_alignment * 0.22);
    }
    
    // Optional microscopic frost noise (if roughness > 0)
    if (params.roughness > 0.0) {
        let noise = fract(sin(dot(uv + params.time, vec2<f32>(12.9898, 78.233))) * 43758.5453) - 0.5;
        final_color += noise * params.roughness * 0.08;
    }
    
    // Premultiplied alpha output
    return vec4<f32>(final_color * alpha, alpha);
}
