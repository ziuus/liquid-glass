// row_ink.wgsl: per-region luminance sampling with hysteresis (compute pass)
//
// One thread per "region" (a task row, tab bar, chip row, input bar, clock block...).
// Each frame it:
//   1. measures the blurred backdrop under the region (mean + spread of luminance)
//   2. smooths that over time
//   3. runs a Schmitt trigger to pick dark vs light ink
//   4. animates a crossfade so the flip never pops
//   5. suggests a small scrim when the backdrop is busy or near the black/white crossover
//
// State lives in a storage buffer, so the fragment shader only reads the results.
// Hysteresis needs memory across frames, which a fragment shader doesn't have.
// Run this BEFORE the glass render pass, in the same command encoder.
//
// Sample the same blurred, saturation-boosted backdrop the glass interior is built from.
// Never sample the final composite: that creates a feedback loop and flicker.

struct Region {
    rect: vec4<f32>,   // x, y, w, h in backdrop UV: the region's own position, NOT the refracted one
    tint: vec4<f32>,   // rgb (authored as sRGB 0..1) + alpha of the fill between backdrop and text
    misc: vec4<f32>,   // x = mip level to sample; yzw unused
};

// 32 bytes, all f32. Mirror with #[repr(C)] and 8 f32 on the Rust side.
struct RegionState {
    lum: f32,        // smoothed relative luminance (linear light, 0..1)
    spread: f32,     // smoothed std-dev of luminance across the region (backdrop "busyness")
    dark_ink: f32,   // Schmitt trigger state: 1.0 = dark text, 0.0 = light text
    ink_mix: f32,    // animated 0..1 crossfade: the value the fragment shader reads
    scrim: f32,      // suggested local scrim alpha (0..0.2)
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

struct Params {
    count: u32,
    first_frame: u32,   // 1 on startup and after a wallpaper change: snap instead of easing
    dt: f32,            // seconds since last frame
    lum_tau: f32,       // luminance smoothing time constant (s), start at 0.15
    mix_tau: f32,       // ink crossfade time constant (s), start at 0.08
    to_dark: f32,       // 0.21: smoothed lum above this switches to dark ink
    to_light: f32,      // 0.15: smoothed lum below this switches to light ink
    _pad: f32,
};

// Luminance where black and white text have equal WCAG contrast (about 4.6:1).
// Center of the hysteresis band: (0.21 + 0.15) / 2 ~= 0.18.
const CROSSOVER: f32 = 0.179;

// true if the backdrop view already returns linear values when sampled
// (an *Srgb format, or a float format holding linear light).
const BACKDROP_ALREADY_LINEAR: bool = true;

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> regions: array<Region>;
@group(0) @binding(2) var<storage, read_write> states: array<RegionState>;
@group(0) @binding(3) var backdrop_blur: texture_2d<f32>;
@group(0) @binding(4) var backdrop_samp: sampler;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    if (BACKDROP_ALREADY_LINEAR) {
        return c;
    }
    return srgb_to_linear(c);
}

fn rel_lum(lin: vec3<f32>) -> f32 {
    return dot(lin, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.count) {
        return;
    }

    let r = regions[i];
    var s = states[i];

    // 1. Measure: 4x4 taps over the region on the blurred backdrop, with the
    //    known fill composited on top so we measure what the eye actually sees.
    var sum = 0.0;
    var sum2 = 0.0;
    for (var y = 0u; y < 4u; y += 1u) {
        for (var x = 0u; x < 4u; x += 1u) {
            let uv = r.rect.xy + r.rect.zw * (vec2<f32>(f32(x), f32(y)) + 0.5) / 4.0;
            let rgb = textureSampleLevel(backdrop_blur, backdrop_samp, uv, r.misc.x).rgb;
            let seen = mix(to_linear(rgb), srgb_to_linear(r.tint.rgb), r.tint.a);
            let l = rel_lum(seen);
            sum += l;
            sum2 += l * l;
        }
    }
    let mean = sum / 16.0;
    let spread = sqrt(max(sum2 / 16.0 - mean * mean, 0.0));

    // 2. Frame-rate independent smoothing; snap on the first frame.
    let snap = P.first_frame == 1u;
    let a_lum = select(1.0 - exp(-P.dt / P.lum_tau), 1.0, snap);
    s.lum = mix(s.lum, mean, a_lum);
    s.spread = mix(s.spread, spread, a_lum);

    // 3. Schmitt trigger on the smoothed value: the dead band stops flicker
    //    when the backdrop hovers near the switch point.
    if (snap) {
        s.dark_ink = select(0.0, 1.0, s.lum > CROSSOVER);
        s.ink_mix = s.dark_ink;
    } else if (s.dark_ink < 0.5) {
        if (s.lum > P.to_dark) {
            s.dark_ink = 1.0;
        }
    } else {
        if (s.lum < P.to_light) {
            s.dark_ink = 0.0;
        }
    }

    // 4. Animate the flip so it never pops.
    let a_mix = 1.0 - exp(-P.dt / P.mix_tau);
    s.ink_mix = mix(s.ink_mix, s.dark_ink, a_mix);

    // 5. Scrim: when the backdrop is busy, or sits near the crossover where
    //    neither ink gets much above 4.5:1, ask for a soft push toward the
    //    opposite of the ink color. Multiply by 0 to disable.
    let busy = smoothstep(0.06, 0.20, s.spread);
    let ambiguous = 1.0 - smoothstep(0.0, 0.08, abs(s.lum - CROSSOVER));
    s.scrim = 0.20 * max(busy, ambiguous);

    states[i] = s;
}
