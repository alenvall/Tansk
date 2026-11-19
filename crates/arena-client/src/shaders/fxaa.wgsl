// FXAA (after Lottes' FXAA 2 "PC" filter): the cheap anti-aliasing for the
// Medium and Low presets, instead of MSAA. Reads the tonemapped, sRGB-encoded
// frame (at the render scale) and writes the output (at full size), so it
// is also the upscale. Five taps on flat pixels, nine on edges.

struct Fxaa {
    // xy 1 / source size, z decode sRGB to linear on write (1) or not (0), w unused
    params: vec4<f32>,
    // xy 1 / output size
    out: vec4<f32>,
};

@group(0) @binding(0) var<uniform> fx: Fxaa;
@group(0) @binding(1) var ldr: texture_2d<f32>;
@group(0) @binding(2) var ldr_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(index & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn tap(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(ldr, ldr_sampler, uv, 0.0).rgb;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

const REDUCE_MIN: f32 = 1.0 / 128.0;
const REDUCE_MUL: f32 = 1.0 / 8.0;
const SPAN_MAX: f32 = 8.0;

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = pos.xy * fx.out.xy;
    let t = fx.params.xy;
    let m = tap(uv);
    let nw = luma(tap(uv + vec2<f32>(-1.0, -1.0) * t));
    let ne = luma(tap(uv + vec2<f32>(1.0, -1.0) * t));
    let sw = luma(tap(uv + vec2<f32>(-1.0, 1.0) * t));
    let se = luma(tap(uv + vec2<f32>(1.0, 1.0) * t));
    let lm = luma(m);
    let lo = min(lm, min(min(nw, ne), min(sw, se)));
    let hi = max(lm, max(max(nw, ne), max(sw, se)));
    var c = m;
    // Flat or faint: leave it (most of the screen).
    if (hi - lo >= max(0.05, hi * 0.15)) {
        var dir = vec2<f32>(-((nw + ne) - (sw + se)), (nw + sw) - (ne + se));
        let reduce = max((nw + ne + sw + se) * (0.25 * REDUCE_MUL), REDUCE_MIN);
        let rcp_min = 1.0 / (min(abs(dir.x), abs(dir.y)) + reduce);
        dir = clamp(dir * rcp_min, vec2<f32>(-SPAN_MAX), vec2<f32>(SPAN_MAX)) * t;
        let a = 0.5 * (tap(uv + dir * (1.0 / 3.0 - 0.5)) + tap(uv + dir * (2.0 / 3.0 - 0.5)));
        let b = a * 0.5 + 0.25 * (tap(uv - dir * 0.5) + tap(uv + dir * 0.5));
        let lb = luma(b);
        c = select(b, a, lb < lo || lb > hi);
    }
    if (fx.params.z > 0.5) {
        c = srgb_to_linear(c);
    }
    return vec4<f32>(c, 1.0);
}
