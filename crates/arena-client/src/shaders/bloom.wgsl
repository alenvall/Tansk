// Bloom: exposure and a soft-threshold prefilter into half resolution, a
// chain of bilinear downsamples, then tent-filtered upsamples added back up
// the chain, each wider level scaled by `scatter` (a tight glow). The
// tonemap adds the top of the chain to the frame.

struct Bloom {
    // x threshold, y soft knee (fraction of the threshold), z upsample radius (texels), w exposure
    params: vec4<f32>,
    // x scatter (how much each wider level adds)
    params2: vec4<f32>,
};

@group(0) @binding(0) var<uniform> bloom: Bloom;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> Out {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var out: Out;
    out.clip = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

// 13-tap downsample (Jimenez 2014): smooth and stable under motion.
fn downsample(uv: vec2<f32>) -> vec3<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(source));
    let a = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(-2.0, -2.0), 0.0).rgb;
    let b = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(0.0, -2.0), 0.0).rgb;
    let c = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(2.0, -2.0), 0.0).rgb;
    let d = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(-2.0, 0.0), 0.0).rgb;
    let e = textureSampleLevel(source, linear_sampler, uv, 0.0).rgb;
    let f = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(2.0, 0.0), 0.0).rgb;
    let g = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(-2.0, 2.0), 0.0).rgb;
    let h = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(0.0, 2.0), 0.0).rgb;
    let i = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(2.0, 2.0), 0.0).rgb;
    let j = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(-1.0, -1.0), 0.0).rgb;
    let k = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(1.0, -1.0), 0.0).rgb;
    let l = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(-1.0, 1.0), 0.0).rgb;
    let m = textureSampleLevel(source, linear_sampler, uv + t * vec2<f32>(1.0, 1.0), 0.0).rgb;
    return e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
}

@fragment
fn fs_prefilter(in: Out) -> @location(0) vec4<f32> {
    // Exposed first, so the threshold is in the frame's terms; clamped
    // against fireflies (a single hot texel blooming into a disc).
    let c = min(downsample(in.uv) * bloom.params.w, vec3<f32>(16.0));
    let brightness = max(c.r, max(c.g, c.b));
    let threshold = bloom.params.x;
    let knee = threshold * bloom.params.y + 1e-4;
    var soft = clamp(brightness - threshold + knee, 0.0, 2.0 * knee);
    soft = soft * soft / (4.0 * knee);
    let contribution = max(soft, brightness - threshold) / max(brightness, 1e-4);
    return vec4<f32>(c * contribution, 1.0);
}

@fragment
fn fs_down(in: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(downsample(in.uv), 1.0);
}

// 9-tap tent upsample of the smaller level, added (by blending) onto the
// larger one.
@fragment
fn fs_up(in: Out) -> @location(0) vec4<f32> {
    let t = bloom.params.z / vec2<f32>(textureDimensions(source));
    var c = textureSampleLevel(source, linear_sampler, in.uv, 0.0).rgb * 4.0;
    c = c + textureSampleLevel(source, linear_sampler, in.uv + t * vec2<f32>(-1.0, 0.0), 0.0).rgb * 2.0;
    c = c + textureSampleLevel(source, linear_sampler, in.uv + t * vec2<f32>(1.0, 0.0), 0.0).rgb * 2.0;
    c = c + textureSampleLevel(source, linear_sampler, in.uv + t * vec2<f32>(0.0, -1.0), 0.0).rgb * 2.0;
    c = c + textureSampleLevel(source, linear_sampler, in.uv + t * vec2<f32>(0.0, 1.0), 0.0).rgb * 2.0;
    c = c + textureSampleLevel(source, linear_sampler, in.uv + t * vec2<f32>(-1.0, -1.0), 0.0).rgb;
    c = c + textureSampleLevel(source, linear_sampler, in.uv + t * vec2<f32>(1.0, -1.0), 0.0).rgb;
    c = c + textureSampleLevel(source, linear_sampler, in.uv + t * vec2<f32>(-1.0, 1.0), 0.0).rgb;
    c = c + textureSampleLevel(source, linear_sampler, in.uv + t * vec2<f32>(1.0, 1.0), 0.0).rgb;
    return vec4<f32>(c * (bloom.params2.x / 16.0), 1.0);
}
