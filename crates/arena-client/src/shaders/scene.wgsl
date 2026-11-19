// TANSK scene shading: illustrative, after "Illustrative Rendering in Team
// Fortress 2" (Mitchell, Francke, Eng, 2007), on cascaded soft shadows, in
// a hot, hazy desert light.
//
//   view-independent:  k_d * ( a(n) + c * warp( (0.5 n.l + 0.5)^2 * shadow ) )
//   view-dependent:    c * k_s * max( f_s (v.r)^k_spec, f_r k_r (v.r)^k_rim )
//                      + (n.up) * f_r * k_r * a(v)
//
// The air: distance haze plus an exponential height fog, integrated along
// the view ray, lit by the sky and by the sun through a Henyey-Greenstein
// forward-scattering lobe, so the low sun glows through the dust.
//
// Materials (per vertex, `params.y`): 0 painted steel, 1 sand, 2 bare
// steel, 3 rubber (treads), 4 rock, 5 team paint, 6 wood, 7 matte,
// 8 hazard stripes, 9 asphalt, 10 glow, 11 ground decal. `params.x` roughness, `params.z`
// wear. Per instance `iparams`: x/z tread scroll (left/right), y hit flash.

const PI: f32 = 3.14159265;
const MAX_OCCLUDERS: u32 = 32u;
const MAX_LIGHTS: u32 = 8u;

struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cascades: array<mat4x4<f32>, 3>,
    // xyz eye, w time
    camera: vec4<f32>,
    // w: a pixel's size (m) at 1 m from the eye
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    sky_color: vec4<f32>,
    ground_color: vec4<f32>,
    // rgb horizon haze, w haze start distance
    fog: vec4<f32>,
    // x haze density / m, y height-fog density at y = 0, z height falloff / m, w Mie g
    fog2: vec4<f32>,
    // x map resolution, y PCF spacing (texels), z sun glow strength in the air, w sandstorm 0..1
    shadow: vec4<f32>,
    // cascade far distances (xyz), w cascades in use (2 or 3)
    splits: vec4<f32>,
    // texel world size per cascade (xyz), w lite shading (Low preset) 1 / 0
    texels: vec4<f32>,
    // light depth range per cascade (xyz), w blue sky gain
    ranges: vec4<f32>,
    // x occluder count, y light count, z rim strength, w spec strength
    misc: vec4<f32>,
    occluders: array<vec4<f32>, 32>,
    light_pos: array<vec4<f32>, 8>,
    light_col: array<vec4<f32>, 8>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var shadow_map: texture_depth_2d_array;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) params: vec4<f32>,
};

struct InstanceIn {
    @location(4) m0: vec4<f32>,
    @location(5) m1: vec4<f32>,
    @location(6) m2: vec4<f32>,
    @location(7) m3: vec4<f32>,
    @location(8) iparams: vec4<f32>,
    @location(9) tint: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) params: vec3<f32>,
    @location(4) local: vec3<f32>,
    @location(5) iparams: vec4<f32>,
    @location(6) tint: vec4<f32>,
};

@vertex
fn vs_main(v: VertexIn, i: InstanceIn) -> VertexOut {
    let model = mat4x4<f32>(i.m0, i.m1, i.m2, i.m3);
    var world = model * vec4<f32>(v.position, 1.0);
    var out: VertexOut;
    out.normal = (model * vec4<f32>(v.normal, 0.0)).xyz;
    out.params = v.params.xyz;
    // Wires (bare steel with params.z = -radius): a thin tube breaks into
    // dots once it is under a pixel or two wide, so it is pushed out along
    // its normals to at least 1.5 px; the fragment fades it towards the air
    // by how much it was widened (params.z then carries that coverage).
    if (u32(v.params.y + 0.5) == 2u && v.params.z < 0.0) {
        let r = -v.params.z;
        let want = 0.75 * globals.cam_right.w * length(world.xyz - globals.camera.xyz);
        let grow = max(want - r, 0.0);
        world = vec4<f32>(world.xyz + normalize(out.normal) * grow, 1.0);
        out.params.z = -clamp(r / max(want, 1e-5), 0.0, 1.0) - 1.0;
    }
    out.clip = globals.view_proj * world;
    out.world = world.xyz;
    out.color = v.color;
    // Merged chunks carry each piece's ground height in params.w, so the
    // shader's "height above the ground" (local y) still holds.
    out.local = v.position + vec3<f32>(0.0, v.params.w, 0.0);
    out.iparams = i.iparams;
    out.tint = i.tint;
    return out;
}

// The shadow pass: one cascade's matrix in its own uniform.
@group(1) @binding(0) var<uniform> cascade_vp: mat4x4<f32>;

@vertex
fn vs_shadow(v: VertexIn, i: InstanceIn) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(i.m0, i.m1, i.m2, i.m3);
    return cascade_vp * model * vec4<f32>(v.position, 1.0);
}

// ── Noise (broad strokes only) ──────────────────────────────────────────

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

fn hash13(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 = p3 + dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash13(i), hash13(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash13(i + vec3<f32>(0.0, 1.0, 0.0)), hash13(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash13(i + vec3<f32>(0.0, 0.0, 1.0)), hash13(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash13(i + vec3<f32>(0.0, 1.0, 1.0)), hash13(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

fn blotch3(p: vec3<f32>) -> f32 {
    return 0.7 * noise3(p) + 0.3 * noise3(p * 2.7 + 11.0);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash12(i), hash12(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash12(i + vec2<f32>(0.0, 1.0)), hash12(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

fn fbm2(p: vec2<f32>) -> f32 {
    return 0.55 * noise2(p) + 0.3 * noise2(p * 2.1 + 5.2) + 0.15 * noise2(p * 4.3 + 1.7);
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn paint_edge(x: f32, at: f32, soft: f32) -> f32 {
    return smoothstep(at - soft, at + soft, x);
}

// Voronoi edge distance (cell borders), for cracks.
fn voronoi_edge(p: vec2<f32>) -> f32 {
    let ci = floor(p);
    var d1 = 9.0;
    var d2 = 9.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let o = ci + vec2<f32>(f32(x), f32(y));
            let q = o + hash22(o) - p;
            let d = dot(q, q);
            if (d < d1) {
                d2 = d1;
                d1 = d;
            } else if (d < d2) {
                d2 = d;
            }
        }
    }
    return sqrt(d2) - sqrt(d1);
}

// ── Materials ───────────────────────────────────────────────────────────

struct Surface {
    albedo: vec3<f32>,
    spec_power: f32,
    spec: f32,
    rim: f32,
    emissive: vec3<f32>,
    // Normal tweak (world space), added before normalising.
    bump: vec3<f32>,
};

fn surface(albedo: vec3<f32>, spec_power: f32, spec: f32, rim: f32) -> Surface {
    var s: Surface;
    s.albedo = albedo;
    s.spec_power = spec_power;
    s.spec = spec;
    s.rim = rim;
    s.emissive = vec3<f32>(0.0);
    s.bump = vec3<f32>(0.0);
    return s;
}

const SAND: vec3<f32> = vec3<f32>(0.60, 0.42, 0.25);
const DUST: vec3<f32> = vec3<f32>(0.62, 0.48, 0.33);

fn sand_ground(p: vec3<f32>, fw: vec3<f32>) -> Surface {
    let xz = p.xz;
    // Broad painted variation: paler crests, warmer hollows.
    let big = fbm2(xz * 0.012);
    let mid = noise2(xz * 0.09 + 7.0);
    var c = mix(SAND * vec3<f32>(0.9, 0.86, 0.82), SAND * vec3<f32>(1.12, 1.06, 0.98), big);
    c = c * (0.95 + 0.1 * mid);

    // Wind ripples on loose sand: a warped sine, as a bump and a soft tone.
    let w = max(fw.x, fw.z) + 1e-4;
    let warp = fbm2(xz * 0.08) * 6.0;
    let phase = dot(xz, vec2<f32>(0.8, 0.6)) * 2.2 + warp;
    let ripple_fade = (1.0 - smoothstep(0.08, 0.25, w)) * smoothstep(0.35, 0.6, fbm2(xz * 0.02 + 3.0));
    let ripple = sin(phase) * ripple_fade;

    // Cracked clay pans, only where the noise says so, at varying scales.
    let pan = paint_edge(fbm2(xz * 0.018 + vec2<f32>(4.7, 1.3)), 0.6, 0.04);
    let scale = mix(0.45, 0.8, noise2(xz * 0.01));
    let warp2 = vec2<f32>(noise2(xz * 0.3), noise2(xz * 0.3 + 9.0)) * 0.6;
    let edge = voronoi_edge(xz * scale + warp2);
    let cw = 0.02 + 0.03 * noise2(xz * 0.5);
    let crack = (1.0 - smoothstep(cw, cw + w * scale * 1.5, edge)) * (1.0 - smoothstep(0.05, 0.12, w));
    let clay = mix(c, vec3<f32>(0.70, 0.54, 0.38), 0.5);
    c = mix(c, clay, pan);
    c = mix(c, c * 0.78, crack * pan);
    c = c * (1.0 + 0.05 * ripple * (1.0 - pan));

    var s = surface(c, 4.0, 0.03, 0.2);
    let dir = normalize(vec2<f32>(0.8, 0.6));
    let slope = cos(phase) * ripple_fade * 0.12 * (1.0 - pan);
    s.bump = vec3<f32>(dir.x * slope, 0.0, dir.y * slope);
    return s;
}

// The Refinery's salt flat: the sand washed pale and crusted, with a big
// polygon crack net and soft grey-rust stains, `salt` 0..1.
fn salt_flat(base: Surface, p: vec3<f32>, fw: vec3<f32>, salt: f32) -> Surface {
    var s = base;
    let xz = p.xz;
    let w = max(fw.x, fw.z) + 1e-4;
    // Pale, slightly warm crust in broad patches; the sand shows between.
    let crusty = smoothstep(0.3, 0.7, fbm2(xz * 0.011 + 2.0));
    let crust = vec3<f32>(0.76, 0.70, 0.61) * (0.92 + 0.12 * fbm2(xz * 0.03));
    var c = mix(base.albedo, crust, salt * (0.3 + 0.45 * crusty));
    // Irregular crust plates with darker rims, only where the crust is.
    // Like the Sink's clay: the plate size drifts across the flat, the rims
    // wander at two scales, their width varies and some fade out, and a
    // finer net of hairline cracks splits the bigger plates, so up close
    // it reads as dried crust, not tiles.
    let scale = mix(0.12, 0.22, noise2(xz * 0.013 + 3.0));
    let warp = vec2<f32>(noise2(xz * 0.11), noise2(xz * 0.11 + 5.0)) * 1.4
        + vec2<f32>(noise2(xz * 0.47 + 2.0), noise2(xz * 0.47 + 7.0)) * 0.22;
    let edge = voronoi_edge(xz * scale + warp);
    let cw = (0.012 + 0.03 * noise2(xz * 0.35 + 1.0)) * smoothstep(0.15, 0.45, noise2(xz * 0.06 + 11.0));
    let near = 1.0 - smoothstep(0.1, 0.3, w);
    let rim = (1.0 - smoothstep(cw, cw + w * scale * 2.5, edge)) * near * crusty;
    let fine_s = scale * 2.6;
    let fine_edge = voronoi_edge(xz * fine_s + warp * 1.7 + 13.0);
    let fine = (1.0 - smoothstep(0.012, 0.012 + w * fine_s * 2.5, fine_edge))
        * smoothstep(0.45, 0.7, noise2(xz * 0.09 + 4.0)) * (1.0 - smoothstep(0.05, 0.15, w)) * crusty;
    c = mix(c, c * 0.8, rim * salt);
    c = mix(c, c * 0.88, fine * salt * 0.8);
    // A faint mottle across the crust.
    c = c * (1.0 + 0.05 * salt * crusty * (noise2(xz * 0.9) - 0.5));
    // Oily grey-rust stains round the plant.
    let stain = paint_edge(fbm2(xz * 0.018 + vec2<f32>(9.0, 3.0)), 0.62, 0.08);
    c = mix(c, vec3<f32>(0.40, 0.33, 0.27), stain * salt * 0.5);
    s.albedo = c;
    s.bump = s.bump * (1.0 - salt * 0.6 * crusty);
    return s;
}

// `detail` 1 up close .. 0 once the fine wear (stipple, chips) is smaller
// than a pixel: it fades to its average instead of sparkling.
fn painted_steel(base: vec3<f32>, wear: f32, lp: vec3<f32>, world_y: f32, n: vec3<f32>, edge: f32, camo: f32, damage: f32, detail: f32) -> Surface {
    // Painted, not plastic: broad brush drift and a faint stipple.
    let drift = blotch3(lp * 0.45);
    let stipple = mix(0.5, noise3(lp * 9.0), detail);
    var paint = base * (0.88 + 0.22 * drift) * (0.96 + 0.08 * stipple);
    // Sun-bleached a little (less for team paint, which must read), and
    // team paint a touch more saturated so it separates from the sand.
    paint = mix(paint, vec3<f32>(luminance(paint)) * 1.12, (0.1 + 0.15 * max(n.y, 0.0)) * (1.0 - 0.8 * camo));
    paint = max(mix(vec3<f32>(luminance(paint)), paint, 1.0 + 0.2 * camo), vec3<f32>(0.0));
    // Two-tone camo: a darker, browner shade in big soft blobs.
    let blob = paint_edge(blotch3(lp * 0.55 + vec3<f32>(13.0, 3.0, 7.0)), 0.52, 0.07);
    paint = mix(paint, paint * vec3<f32>(0.58, 0.46, 0.4), blob * camo * 0.35);

    let streak = noise3(vec3<f32>(lp.x * 3.0 + lp.z * 3.0, lp.y * 0.35, 0.0)) * (1.0 - abs(n.y));
    let rust_field = blotch3(lp * 0.8 + 5.0) + 0.25 * wear - 0.2 + 0.2 * streak + 0.15 * (1.0 - smoothstep(0.0, 1.2, world_y));
    let rust = paint_edge(rust_field, 0.8, 0.03) * (1.0 - camo);
    let rust_color = mix(vec3<f32>(0.20, 0.10, 0.06), vec3<f32>(0.34, 0.16, 0.08), noise3(lp * 1.7));
    var c = mix(paint, rust_color, rust);

    // Worn edges: the bevel bands chipped to bright bare metal, broken up
    // so they read as hand-painted wear, plus worn corners.
    let chip = mix(0.3, noise3(lp * 6.0), detail);
    let worn = paint_edge(edge * 3.4 + 0.35 * chip + 0.25 * wear, 0.55, 0.05 + 0.2 * (1.0 - detail));
    c = mix(c, vec3<f32>(0.78, 0.74, 0.68), worn * 0.9);

    // Dust: caked low down and on the running gear, settled on top faces.
    let dust_top = paint_edge(n.y + 0.35 * blotch3(lp * 0.6 + 2.0), 0.88, 0.08);
    let dust_low = 1.0 - smoothstep(0.2, 1.1, world_y + 0.35 * noise3(lp * 2.0));
    let dust = max(dust_top * mix(0.55, 0.15, camo), dust_low * 0.6);
    c = mix(c, DUST, dust);

    // Battle damage: paint knocked off to bare metal, then soot and
    // scorch spreading as the machine nears the end.
    var bump = vec3<f32>(0.0);
    var spec_kill = 0.0;
    if (damage > 0.02) {
        let chips = paint_edge(blotch3(lp * 1.6 + vec3<f32>(17.0, 5.0, 3.0)) + damage * 0.55, 0.88, 0.04);
        c = mix(c, vec3<f32>(0.5, 0.48, 0.45) * (0.8 + 0.3 * mix(0.5, noise3(lp * 7.0), detail)), chips);
        let scorch = paint_edge(blotch3(lp * 0.6 + vec3<f32>(31.0, 2.0, 9.0)) + damage * 0.75, 1.02, 0.1);
        let soot = vec3<f32>(0.055, 0.05, 0.045) * (0.8 + 0.4 * noise3(lp * 4.0));
        c = mix(c, soot, scorch);
        spec_kill = scorch;
        // Dents: the normal pushed about by a coarse noise field.
        let e = 0.15;
        let q = lp * 1.8;
        let h0 = noise3(q);
        bump = vec3<f32>(noise3(q + vec3<f32>(e, 0.0, 0.0)) - h0, noise3(q + vec3<f32>(0.0, e, 0.0)) - h0, noise3(q + vec3<f32>(0.0, 0.0, e)) - h0) * damage * 1.6;
    }
    var s = surface(c, 7.0, (0.08 + 1.1 * worn) * (1.0 - rust * 0.8) * (1.0 - dust * 0.8) * (1.0 - spec_kill), 0.9 * (1.0 - dust * 0.5));
    s.bump = bump;
    return s;
}

// Faceted rock: painted strata following height, every flat facet its own
// slightly different tone (so the planes read), pale worn ridges.
fn rock(p: vec3<f32>, n: vec3<f32>, face: vec3<f32>, edge: f32) -> Surface {
    let y = p.y * 0.42 + 0.6 * noise2(p.xz * 0.05) + 0.2 * noise2(p.xz * 0.3);
    let band = floor(y);
    let f = fract(y);
    let pick = hash12(vec2<f32>(band, 7.0));
    var c = vec3<f32>(0.40, 0.10, 0.035);
    c = select(c, vec3<f32>(0.56, 0.17, 0.05), pick > 0.3);
    c = select(c, vec3<f32>(0.68, 0.30, 0.11), pick > 0.68);
    c = select(c, vec3<f32>(0.47, 0.13, 0.04), pick > 0.88);
    c = c * (0.84 + 0.16 * smoothstep(0.0, 0.4, f));
    // Per-facet tone from the quantised face normal.
    let facet = hash13(floor(face * 9.0 + 0.5) + floor(p * 0.05));
    c = c * (0.86 + 0.24 * facet);
    c = mix(c, DUST, paint_edge(n.y, 0.8, 0.08) * 0.4);
    return surface(c, 4.0, 0.05, 0.5);
}

fn wood(base: vec3<f32>, lp: vec3<f32>, n: vec3<f32>, edge: f32) -> Surface {
    let plank = floor(lp.y * 7.0 + 0.5);
    let tone = 0.8 + 0.35 * hash12(vec2<f32>(plank, 3.0));
    let groove = 1.0 - 0.35 * (1.0 - smoothstep(0.0, 0.08, abs(fract(lp.y * 7.0 + 0.5) - 0.5) * 2.0 - 0.85));
    let grain = 0.92 + 0.12 * noise3(vec3<f32>(lp.x * 2.0, lp.y * 40.0, lp.z * 2.0));
    var c = base * tone * grain * select(1.0, groove, abs(n.y) < 0.5);
    c = mix(c, vec3<f32>(0.78, 0.6, 0.42), paint_edge(edge * 3.0, 0.45, 0.06) * 0.6);
    c = mix(c, DUST, paint_edge(n.y, 0.8, 0.1) * 0.4);
    return surface(c, 6.0, 0.05, 0.5);
}

fn asphalt(lp: vec3<f32>, world: vec3<f32>, fw: vec3<f32>) -> Surface {
    let w = max(fw.x, fw.z) + 1e-4;
    var c = vec3<f32>(0.13, 0.125, 0.12) * (0.85 + 0.3 * fbm2(world.xz * 0.15));
    // Lane markings: a dashed centre line and solid edge lines.
    let az = abs(lp.z);
    let dash = step(fract(lp.x / 8.0 + 0.25), 0.5);
    let centre = (1.0 - smoothstep(0.1, 0.1 + w, az)) * dash;
    let edge_line = (1.0 - smoothstep(0.1, 0.1 + w, abs(az - 5.7)));
    let paint = vec3<f32>(0.82, 0.78, 0.68) * (0.75 + 0.25 * noise2(world.xz * 0.8));
    let worn = smoothstep(0.35, 0.7, noise2(world.xz * 0.4));
    c = mix(c, paint, max(centre * vec3<f32>(1.0, 0.85, 0.35).x, edge_line) * (1.0 - 0.6 * worn));
    c = mix(c, vec3<f32>(0.85, 0.68, 0.2), centre * (1.0 - 0.6 * worn) * 0.6);
    // Cracks and patched tar.
    let crack = (1.0 - smoothstep(0.015, 0.015 + w, voronoi_edge(world.xz * 0.35))) * smoothstep(0.4, 0.7, noise2(world.xz * 0.1));
    c = mix(c, vec3<f32>(0.04), crack * 0.8);
    // Sand blown onto the edges and in drifts.
    let drift = smoothstep(4.6, 6.6, az + 1.5 * noise2(world.xz * 0.3)) + smoothstep(0.62, 0.8, fbm2(world.xz * 0.06));
    c = mix(c, SAND * 1.05, clamp(drift, 0.0, 1.0) * 0.85);
    return surface(c, 10.0, 0.06, 0.25);
}

// ── Lighting ────────────────────────────────────────────────────────────

// The TF2 warp: shade goes cool and a little violet, the terminator warm
// and saturated, the lit side golden.
fn warp(x: f32) -> vec3<f32> {
    let cool = vec3<f32>(0.20, 0.15, 0.20);
    let terminator = vec3<f32>(0.64, 0.43, 0.29);
    let lit = vec3<f32>(1.0, 0.94, 0.84);
    let a = mix(cool, terminator, smoothstep(0.08, 0.42, x));
    return mix(a, lit, smoothstep(0.36, 0.85, x));
}

fn ambient(n: vec3<f32>) -> vec3<f32> {
    let up = globals.sky_color.rgb;
    let down = globals.ground_color.rgb;
    let side = mix(up, down, 0.55) * 1.05;
    let y = n.y;
    // A little extra warm bounce on faces turned towards the sun's side.
    let toward = max(dot(normalize(vec3<f32>(globals.sun_dir.x, 0.0, globals.sun_dir.z)), -n), 0.0);
    let base = select(mix(side, down, -y), mix(side, up, y), y >= 0.0);
    return base + down * 0.35 * toward * (1.0 - abs(y));
}

fn cascade_visibility(layer: i32, world: vec3<f32>, n: vec3<f32>, n_dot_l: f32, texel: f32, range: f32) -> vec2<f32> {
    let res = globals.shadow.x;
    let facing = max(n_dot_l, 0.1);
    let tangent = min(sqrt(max(1.0 - facing * facing, 0.0)) / facing, 3.0);
    let lp = globals.cascades[layer] * vec4<f32>(world + n * texel * 1.8, 1.0);
    let ndc = lp.xyz / lp.w;
    if (ndc.z <= 0.0 || ndc.z >= 1.0 || any(abs(ndc.xy) >= vec2<f32>(0.995))) {
        return vec2<f32>(1.0, 0.0);
    }
    let uv = vec2<f32>(0.5 * ndc.x + 0.5, 0.5 - 0.5 * ndc.y);
    let spacing = globals.shadow.y;
    let bias = texel * (1.0 + 2.0 * spacing) * (0.4 + tangent) / range;
    let z = ndc.z - bias;
    if (globals.texels.w > 0.5) {
        // Lite: four bilinear-compare taps (a 4x4 texel footprint).
        let o = 0.5 * spacing / res;
        let sum = textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2<f32>(-o, -o), layer, z)
            + textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2<f32>(o, -o), layer, z)
            + textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2<f32>(-o, o), layer, z)
            + textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2<f32>(o, o), layer, z);
        return vec2<f32>(sum * 0.25, 1.0);
    }
    var sum = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let o = vec2<f32>(f32(x), f32(y)) * spacing / res;
            sum = sum + textureSampleCompareLevel(shadow_map, shadow_sampler, uv + o, layer, z);
        }
    }
    return vec2<f32>(sum / 9.0, 1.0);
}

fn sun_visibility(world: vec3<f32>, n: vec3<f32>, n_dot_l: f32) -> f32 {
    let d = length(world - globals.camera.xyz);
    // Past the last cascade in use: lit, and no taps spent.
    let last = select(globals.splits.z, globals.splits.y, globals.splits.w < 2.5);
    let storm = 0.65 * globals.shadow.w;
    if (d >= last) {
        return 1.0;
    }
    var layer = 2;
    var texel = globals.texels.z;
    var range = globals.ranges.z;
    if (d < globals.splits.x) {
        layer = 0;
        texel = globals.texels.x;
        range = globals.ranges.x;
    } else if (d < globals.splits.y) {
        layer = 1;
        texel = globals.texels.y;
        range = globals.ranges.y;
    }
    let v = cascade_visibility(layer, world, n, n_dot_l, texel, range);
    // Fade out towards the end of the last cascade.
    let fade = smoothstep(last * 0.85, last, d);
    // A sandstorm washes the shadows out.
    return mix(mix(v.x, 1.0, fade), 1.0, storm);
}

fn sphere_occlusion(p: vec3<f32>, n: vec3<f32>) -> f32 {
    var vis = 1.0;
    // Lite shading skips it (a loop over every occluder per ground pixel).
    let count = select(min(u32(globals.misc.x), MAX_OCCLUDERS), 0u, globals.texels.w > 0.5);
    for (var i = 0u; i < count; i = i + 1u) {
        let s = globals.occluders[i];
        let di = s.xyz - p;
        let l = length(di);
        if (l > s.w * 4.0) {
            continue;
        }
        let nl = max(dot(n, di / max(l, 1e-4)), 0.0);
        let occ = nl * (s.w * s.w) / max(l * l, 1e-4);
        vis = vis * (1.0 - clamp(occ, 0.0, 1.0) * 0.75);
    }
    return vis;
}

fn point_lights(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var c = vec3<f32>(0.0);
    let count = min(u32(globals.misc.y), MAX_LIGHTS);
    for (var i = 0u; i < count; i = i + 1u) {
        let lp = globals.light_pos[i];
        let d = lp.xyz - p;
        let dist = length(d);
        let k = clamp(1.0 - dist / lp.w, 0.0, 1.0);
        let hl = 0.5 * dot(n, d / max(dist, 1e-3)) + 0.5;
        c = c + globals.light_col[i].rgb * k * k * hl * hl / (1.0 + dist * dist * 0.05);
    }
    return c;
}

// Henyey-Greenstein, scaled so the forward peak stays bounded.
fn hg(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / pow(1.0 + g2 - 2.0 * g * cos_theta, 1.5) * 0.25;
}

// The sky: deep blue overhead, a bright warm horizon, the sun's halo.
fn sky_at(dir: vec3<f32>) -> vec3<f32> {
    let horizon = globals.fog.rgb;
    let gain = globals.ranges.w;
    let mid = vec3<f32>(0.26, 0.46, 0.88) * gain;
    let zenith = vec3<f32>(0.06, 0.15, 0.48) * gain;
    let y = max(dir.y, 0.0);
    var c = mix(horizon, mid, smoothstep(0.0, 0.16, pow(y, 0.8)));
    c = mix(c, zenith, smoothstep(0.15, 0.9, y));
    c = select(c, horizon, dir.y < 0.0);
    let toward = max(dot(dir, globals.sun_dir.xyz), 0.0);
    // A modest halo round the sun, warmer near the horizon.
    c = c + vec3<f32>(1.0, 0.74, 0.45) * (0.35 * pow(toward, 16.0) * (1.0 - 0.5 * y) + 0.7 * pow(toward, 120.0));
    c = c + vec3<f32>(1.0, 0.88, 0.7) * 40.0 * smoothstep(0.99935, 0.9996, toward) * (1.0 - globals.shadow.w);
    // Storm: the sky turns to amber dust.
    return mix(c, horizon * (0.95 + 0.15 * y), min(globals.shadow.w * 1.35, 1.0));
}

// How much air lies along a view ray of length `d` from the eye: distance
// haze plus the height fog integral. Returns (transmittance-ish amount 0..1).
fn air_amount(eye: vec3<f32>, dir: vec3<f32>, d: f32) -> f32 {
    // Distance haze ramps in softly past its start (quadratic), so the near
    // and middle distance stay crisp and far mesas soften without vanishing.
    let far = max(d - globals.fog.w, 0.0);
    let haze = globals.fog2.x * far * far / (far + 400.0);
    let a = globals.fog2.y;
    let b = globals.fog2.z;
    let h0 = eye.y;
    var height = 0.0;
    if (abs(dir.y) > 1e-3) {
        height = a * exp(-b * h0) * (1.0 - exp(-b * dir.y * d)) / (b * dir.y);
    } else {
        height = a * exp(-b * h0) * d;
    }
    return 1.0 - exp(-(haze + max(height, 0.0)));
}

// What the air glows with towards `dir`: the horizon colour and the sun
// shining through the dust.
fn air_colour(dir: vec3<f32>) -> vec3<f32> {
    let mu = dot(dir, globals.sun_dir.xyz);
    let mie = hg(mu, globals.fog2.w);
    let flat_dir = normalize(vec3<f32>(dir.x, 0.0, dir.z) + vec3<f32>(0.0, 1e-4, 0.0));
    let along = max(dot(flat_dir, normalize(vec3<f32>(globals.sun_dir.x, 0.0, globals.sun_dir.z))), 0.0);
    return globals.fog.rgb * (0.85 + 0.25 * along) + globals.sun_color.rgb * mie * globals.shadow.z;
}

@fragment
fn fs_main(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let fw = fwidth(in.world);
    let face_raw = cross(dpdx(in.world), dpdy(in.world));

    var n = normalize(in.normal);
    if (!front) {
        n = -n;
    }
    let face = select(n, face_raw * inverseSqrt(max(dot(face_raw, face_raw), 1e-30)), dot(face_raw, face_raw) > 1e-14);
    var edge = 1.0 - abs(dot(n, face));
    let material = u32(in.params.y + 0.5);
    // How big a pixel is here (m): fine detail fades out as it shrinks
    // below a pixel or two, so far props read as clean paint, not sparkle.
    let px = max(max(fw.x, fw.y), fw.z);
    let lite = globals.texels.w > 0.5;
    let detail = select(1.0 - smoothstep(0.012, 0.045, px), 0.0, lite);
    // Bevel bands (the edge-wear signal) go sub-pixel a little further out.
    edge = edge * (1.0 - smoothstep(0.02, 0.06, px));
    // Painted corrugated sheets (params.x = 100 front / 300 back + wave
    // cycles across): the vertex normal is the sheet's; rebuild the wave's
    // normal from the phase, and fade it to flat once a wave gets thinner
    // than about three pixels, so far container stacks read as clean paint.
    let dcx = dpdx(in.params.x);
    let dcy = dpdy(in.params.x);
    let dpx = dpdx(in.world);
    let dpy = dpdy(in.world);
    if (in.params.x > 50.0 && (material == 0u || material == 5u)) {
        let back = in.params.x > 200.0;
        let cyc = in.params.x - select(100.0, 300.0, back);
        let r1 = cross(dpy, face);
        let r2 = cross(face, dpx);
        let det = dot(dpx, r1);
        let g = (dcx * r1 + dcy * r2) / select(det, 1e-8, abs(det) < 1e-12);
        let gl = length(g);
        let t = g / max(gl, 1e-6);
        let per_px = max(max(fw.x, fw.y), fw.z) * gl;
        let fade = 1.0 - smoothstep(0.1, 0.33, per_px);
        let full = normalize(n - select(1.0, -1.0, back) * 0.8 * cos(6.2831853 * cyc) * t);
        edge = (1.0 - abs(dot(full, face))) * fade;
        n = normalize(mix(n, full, fade));
    }
    let wear = in.params.z;
    let v = normalize(globals.camera.xyz - in.world);
    let l = globals.sun_dir.xyz;

    // Height above the ground in the mesh's own frame: props stand on
    // y = 0; tank parts live in the hull frame, 0.95 m up.
    let height = select(in.local.y, in.local.y + 0.95, material == 5u || material == 3u);
    var s: Surface;
    var ao = 1.0;
    if (material == 1u) {
        s = sand_ground(in.world, fw);
        if (wear > 0.0) {
            s = salt_flat(s, in.world, fw, wear);
        }
        // The map's ground tint (white but on the Rust Belt's red canyon).
        s.albedo = s.albedo * in.color;
        ao = sphere_occlusion(in.world, n);
    } else if (material == 0u) {
        s = painted_steel(in.color, wear, in.local, height, n, edge, 0.0, 0.0, detail);
    } else if (material == 5u) {
        // Team paint takes the instance's colour when it carries one.
        let team = select(in.color, in.tint.rgb, in.tint.a > 0.5);
        s = painted_steel(team, wear, in.local, height, n, edge, 1.0, in.iparams.w, detail);
    } else if (material == 4u) {
        s = rock(in.world, n, face, edge);
        ao = sphere_occlusion(in.world, n);
    } else if (material == 6u) {
        s = wood(in.color, in.local, n, edge);
    } else if (material == 7u) {
        let dust = (1.0 - smoothstep(0.0, 0.5, height)) * 0.4;
        s = surface(mix(in.color * (0.85 + 0.3 * blotch3(in.local * 1.2)), DUST, dust), 6.0, 0.06, 0.7);
    } else if (material == 8u) {
        let stripe = fract((in.local.x + in.local.y) * 1.4);
        var c = mix(vec3<f32>(0.95, 0.62, 0.04), vec3<f32>(0.035, 0.03, 0.028), smoothstep(0.48, 0.52, stripe));
        let worn = paint_edge(edge * 3.0 + 0.3 * noise3(in.local * 6.0), 0.45, 0.05);
        c = mix(c, vec3<f32>(0.62, 0.6, 0.56), worn * 0.7);
        c = mix(c, DUST, paint_edge(n.y, 0.8, 0.1) * 0.5);
        s = surface(c, 10.0, 0.15 + worn * 0.6, 0.8);
    } else if (material == 9u) {
        s = asphalt(in.local, in.world, fw);
        ao = sphere_occlusion(in.world, n);
    } else if (material == 11u) {
        // Ground decal (tracks, packed ground, oil): the vertex colour lit
        // like the sand it lies on (sand's rim and specular), so marks
        // do not read paler than the ground from a low camera.
        s = surface(in.color * (0.94 + 0.12 * noise2(in.world.xz * 0.7)), 4.0, 0.03, 0.2);
        ao = sphere_occlusion(in.world, n);
    } else if (material == 10u) {
        s = surface(in.color * 0.2, 4.0, 0.0, 0.0);
        s.emissive = in.color;
    } else if (material == 2u) {
        let dust = paint_edge(n.y, 0.8, 0.1) * 0.35;
        s = surface(mix(in.color * (0.9 + 0.2 * blotch3(in.local * 0.8)), DUST, dust), mix(30.0, 6.0, in.params.x), (1.0 - in.params.x) * 0.8 * (1.0 - dust) + 0.6 * paint_edge(edge * 3.0, 0.3, 0.1), 1.0);
    } else {
        // Rubber treads: grooves that scroll with the track speed.
        let scroll = select(in.iparams.x, in.iparams.z, in.local.z > 0.0);
        let along = in.local.x + scroll * sign(in.local.y + 0.5);
        let groove = smoothstep(0.35, 0.5, abs(fract(along * 4.0) - 0.5) * 2.0);
        let tread_face = 1.0 - abs(n.z);
        var c = in.color * mix(1.0, 0.45 + 0.75 * groove, tread_face);
        let dust = max(paint_edge(n.y + 0.3 * blotch3(in.local * 1.5), 0.6, 0.1) * 0.6, (1.0 - smoothstep(-0.95, 0.0, in.local.y)) * 0.7);
        c = mix(c, SAND * 0.85, dust);
        s = surface(c, 6.0, 0.1 * groove, 0.5);
    }
    if (material != 1u && material != 4u && material != 9u && material != 11u) {
        ao = mix(0.6, 1.0, smoothstep(0.0, 1.2, height));
    }
    n = normalize(n + s.bump);
    // No painted surface is whiter than white paint (about 0.75): brighter
    // albedos (white tanks, trailers) blow out in the sun.
    s.albedo = min(s.albedo, vec3<f32>(0.75));

    let n_dot_l = dot(n, l);
    let shadow = sun_visibility(in.world, n, n_dot_l);
    let half_lambert = 0.5 * n_dot_l + 0.5;
    let x = half_lambert * half_lambert * mix(0.1, 1.0, shadow) * mix(0.5, 1.0, smoothstep(-0.1, 0.25, n_dot_l));
    let sun = globals.sun_color.rgb;
    // Ambient occlusion tints warm, not grey: bounce light from sand.
    let ao_tint = mix(vec3<f32>(0.55, 0.36, 0.26), vec3<f32>(1.0), ao);
    let diffuse = ambient(n) * ao_tint + sun * warp(x) + point_lights(in.world, n);
    var color = s.albedo * diffuse;

    let r = reflect(-l, n);
    let v_dot_r = max(dot(v, r), 0.0);
    let f_r = pow(1.0 - max(dot(n, v), 0.0), 4.0);
    let k_r = s.rim * globals.misc.z;
    let phong = s.spec * globals.misc.w * pow(v_dot_r, s.spec_power);
    let rim_lit = f_r * k_r * pow(v_dot_r, 2.0);
    color = color + sun * max(phong, rim_lit) * mix(0.2, 1.0, shadow);
    color = color + max(n.y, 0.0) * f_r * k_r * sky_at(-v) * 0.6 * ao;
    color = color + s.emissive;
    // Hit flash.
    color = mix(color, vec3<f32>(3.0, 2.6, 2.2), clamp(in.iparams.y, 0.0, 1.0));

    let to_frag = in.world - globals.camera.xyz;
    let dist = length(to_frag);
    let dir = to_frag / max(dist, 1e-4);
    var amount = air_amount(globals.camera.xyz, dir, dist);
    if (material == 2u && in.params.z < 0.0) {
        // A widened wire: as thin as it really is, it would cover only this
        // much of its pixels; mostly sky shows round it, so fade to the air.
        let coverage = clamp(-in.params.z - 1.0, 0.0, 1.0);
        amount = 1.0 - (1.0 - amount) * mix(0.45, 1.0, coverage);
    }
    color = mix(color, air_colour(dir), amount);
    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}

struct BgOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_background(@builtin(vertex_index) index: u32) -> BgOut {
    let x = f32((index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(index & 2u) * 2.0 - 1.0;
    var out: BgOut;
    out.clip = vec4<f32>(x, y, 1.0, 1.0);
    out.ndc = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_background(in: BgOut) -> @location(0) vec4<f32> {
    let far = globals.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let near = globals.inv_view_proj * vec4<f32>(in.ndc, 0.0, 1.0);
    let dir = normalize(far.xyz / far.w - near.xyz / near.w);
    // The sky keeps its blue: the dust only veils the band near the horizon.
    let amount = air_amount(globals.camera.xyz, dir, 900.0) * (1.0 - smoothstep(0.0, 0.12, dir.y));
    return vec4<f32>(mix(sky_at(dir), air_colour(dir), amount), 1.0);
}
