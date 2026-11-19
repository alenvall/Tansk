// Composites the half-resolution effects target onto the frame: its rgb is
// premultiplied colour, its alpha what still shows through (the blend does
// frame * alpha + colour). Bilinear, so the upsample stays soft.

@group(0) @binding(0) var fx_tex: texture_2d<f32>;
@group(0) @binding(1) var fx_sampler: sampler;

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

@fragment
fn fs_main(in: Out) -> @location(0) vec4<f32> {
    return textureSampleLevel(fx_tex, fx_sampler, in.uv, 0.0);
}
