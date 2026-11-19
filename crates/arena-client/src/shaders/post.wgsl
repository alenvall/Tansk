// Post: exposure, saturation and warm grade, filmic tonemap (ACES fitted, or AgX), vignette, sRGB encode.

struct Post {
    // x exposure, y vignette, z encode sRGB in shader (1) or not (0), w curve (0 ACES, 1 AgX)
    params: vec4<f32>,
    // rgb warm print tint, w saturation
    grade: vec4<f32>,
    // x bloom strength, y contrast, zw 1 / output size
    bloom: vec4<f32>,
};

@group(0) @binding(0) var<uniform> post: Post;
@group(0) @binding(1) var hdr: texture_2d<f32>;
@group(0) @binding(2) var bloom_tex: texture_2d<f32>;
@group(0) @binding(3) var bloom_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(index & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Narkowicz's fit of the ACES RRT+ODT.
fn aces_fitted(input: vec3<f32>) -> vec3<f32> {
    let x = max(input, vec3<f32>(0.0)) * 0.6;
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

// AgX (minimal, punchy look), from brick-racers' tonemap.wgsl.
fn agx(input: vec3<f32>) -> vec3<f32> {
    let inset = mat3x3<f32>(
        vec3<f32>(0.842479062253094, 0.0423282422610123, 0.0423756549057051),
        vec3<f32>(0.0784335999999992, 0.878468636469772, 0.0784336),
        vec3<f32>(0.0792237451477643, 0.0791661274605434, 0.879142973793104),
    );
    let outset = mat3x3<f32>(
        vec3<f32>(1.19687900512017, -0.0528968517574562, -0.0529716355144438),
        vec3<f32>(-0.0980208811401368, 1.15190312990417, -0.0980434501171241),
        vec3<f32>(-0.0990297440797205, -0.0989611768448433, 1.15107367264116),
    );
    let min_ev = -12.47393;
    let max_ev = 4.026069;
    var c = inset * max(input, vec3<f32>(1e-10));
    c = clamp(log2(c), vec3<f32>(min_ev), vec3<f32>(max_ev));
    c = (c - min_ev) / (max_ev - min_ev);
    let x2 = c * c;
    let x4 = x2 * x2;
    c = 15.5 * x4 * x2 - 40.14 * x4 * c + 31.96 * x4 - 6.868 * x2 * c + 0.4298 * x2
        + 0.1191 * c - 0.00232;
    c = pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.35));
    c = mix(vec3<f32>(luminance(c)), c, 1.4);
    c = outset * max(c, vec3<f32>(0.0));
    return pow(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(2.2));
}

fn linear_to_srgb(linear: vec3<f32>) -> vec3<f32> {
    let c = clamp(linear, vec3<f32>(0.0), vec3<f32>(1.0));
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    let low = c * 12.92;
    return select(high, low, c <= vec3<f32>(0.0031308));
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    // The scene may be rendered at a lower scale: sample it filtered.
    let suv = pos.xy * post.bloom.zw;
    let size = 1.0 / post.bloom.zw;
    var c = textureSampleLevel(hdr, bloom_sampler, suv, 0.0).rgb;
    c = mix(c, textureSampleLevel(bloom_tex, bloom_sampler, suv, 0.0).rgb, post.bloom.x);
    c = c * post.params.x;
    // Contrast round mid grey, hue-preserving.
    let lum = max(luminance(c), 1e-5);
    c = c * (0.18 * pow(lum / 0.18, post.bloom.y) / lum);
    c = max(mix(vec3<f32>(luminance(c)), c, post.grade.w), vec3<f32>(0.0)) * post.grade.rgb;

    var mapped: vec3<f32>;
    if (post.params.w > 0.5) {
        mapped = agx(c);
    } else {
        mapped = aces_fitted(c);
    }

    // Vignette, a touch of lens.
    let uv = pos.xy / size - 0.5;
    let aspect = size.x / size.y;
    let r = length(uv * vec2<f32>(aspect, 1.0)) / length(vec2<f32>(aspect, 1.0) * 0.5);
    mapped = mapped * (1.0 - post.params.y * smoothstep(0.45, 1.05, r));

    if (post.params.z > 0.5) {
        mapped = linear_to_srgb(mapped);
    }
    // Dither away banding in the background gradient.
    let n = fract(sin(dot(pos.xy, vec2<f32>(12.9898, 78.233))) * 43758.5453) - 0.5;
    mapped = mapped + n / 255.0;
    return vec4<f32>(mapped, 1.0);
}
