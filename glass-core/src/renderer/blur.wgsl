// Multi-pass Gaussian blur shaders for Apple-quality liquid glass
// Implements separable blur, saturation boost, and efficient up/downsampling

struct BlurUniforms {
    direction: vec2<f32>,        // [1,0] for horizontal, [0,1] for vertical
    radius: f32,                 // Blur radius in pixels
    saturation_boost: f32,       // Saturation multiplier
    resolution: vec2<f32>,       // Texture resolution
    _padding: vec2<f32>,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: BlurUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Fullscreen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate fullscreen triangle
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);

    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);

    return out;
}

// Downsample with 4-tap bilinear filter
@fragment
fn fs_downsample(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel_size = 1.0 / uniforms.resolution;

    // 4-tap bilinear downsample for better quality
    let uv = in.uv;
    var color = vec4<f32>(0.0);

    color += textureSample(input_texture, input_sampler, uv + vec2<f32>(-0.5, -0.5) * texel_size);
    color += textureSample(input_texture, input_sampler, uv + vec2<f32>(0.5, -0.5) * texel_size);
    color += textureSample(input_texture, input_sampler, uv + vec2<f32>(-0.5, 0.5) * texel_size);
    color += textureSample(input_texture, input_sampler, uv + vec2<f32>(0.5, 0.5) * texel_size);

    return color * 0.25;
}

// Gaussian blur weights for various kernel sizes
// Using optimized 9-tap kernel with linear sampling
@fragment
fn fs_gaussian_blur(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel_size = 1.0 / uniforms.resolution;
    let offset = uniforms.direction * texel_size;

    // Optimized 9-tap Gaussian kernel with linear sampling
    // These weights and offsets are precomputed for a standard Gaussian
    let weights = array<f32, 5>(
        0.2270270270,
        0.1945945946,
        0.1216216216,
        0.0540540541,
        0.0162162162
    );

    // Center sample
    var color = textureSample(input_texture, input_sampler, in.uv) * weights[0];

    // Bilateral samples (both directions)
    for (var i = 1; i < 5; i++) {
        let offset_dist = f32(i) * offset * uniforms.radius * 0.1;
        color += textureSample(input_texture, input_sampler, in.uv + offset_dist) * weights[i];
        color += textureSample(input_texture, input_sampler, in.uv - offset_dist) * weights[i];
    }

    return color;
}

// RGB to HSV conversion
fn rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
    let max_c = max(max(rgb.r, rgb.g), rgb.b);
    let min_c = min(min(rgb.r, rgb.g), rgb.b);
    let delta = max_c - min_c;

    var hsv = vec3<f32>(0.0);
    hsv.z = max_c; // V (value)

    if (max_c > 0.0) {
        hsv.y = delta / max_c; // S (saturation)
    }

    if (delta > 0.0) {
        if (max_c == rgb.r) {
            hsv.x = (rgb.g - rgb.b) / delta;
            if (rgb.g < rgb.b) {
                hsv.x += 6.0;
            }
        } else if (max_c == rgb.g) {
            hsv.x = 2.0 + (rgb.b - rgb.r) / delta;
        } else {
            hsv.x = 4.0 + (rgb.r - rgb.g) / delta;
        }
        hsv.x /= 6.0;
    }

    return hsv;
}

// HSV to RGB conversion
fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let h = hsv.x * 6.0;
    let s = hsv.y;
    let v = hsv.z;

    let i = floor(h);
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    var rgb: vec3<f32>;
    let ii = i32(i) % 6;

    switch ii {
        case 0: { rgb = vec3<f32>(v, t, p); }
        case 1: { rgb = vec3<f32>(q, v, p); }
        case 2: { rgb = vec3<f32>(p, v, t); }
        case 3: { rgb = vec3<f32>(p, q, v); }
        case 4: { rgb = vec3<f32>(t, p, v); }
        default: { rgb = vec3<f32>(v, p, q); }
    }

    return rgb;
}

// Saturation boost - key for Apple's liquid glass look
@fragment
fn fs_saturation_boost(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(input_texture, input_sampler, in.uv);

    // Convert to HSV, boost saturation, convert back
    var hsv = rgb_to_hsv(color.rgb);
    hsv.y = clamp(hsv.y * uniforms.saturation_boost, 0.0, 1.0);
    color = vec4<f32>(hsv_to_rgb(hsv), color.a);

    // Also apply slight contrast boost
    color.r = (color.r - 0.5) * 1.05 + 0.5;
    color.g = (color.g - 0.5) * 1.05 + 0.5;
    color.b = (color.b - 0.5) * 1.05 + 0.5;

    return color;
}

// Bilinear upsample
@fragment
fn fs_upsample(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple bilinear upsample
    // The sampler already does the work
    return textureSample(input_texture, input_sampler, in.uv);
}
