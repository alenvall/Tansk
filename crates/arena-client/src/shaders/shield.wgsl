// Glass: translucent self-lit shells, drawn after the opaque scene and
// before the sprites. Appended to scene.wgsl at load time (its globals,
// hashes and air). Styles (`params.x`):
//   0 shield bubble: a hot amber field round a tank, fresnel rim, welded
//     panels shimmering, a gentle wobble, and ring waves from each hit
//   1 aura bubble round a pickup: a soft rim glow
//   2 aura column: a soft beam rising from a pickup, fading upward
//   3 ground ring under a pickup: a band with a pulse running outward
//   4 hot glow: an additive blob (shield-stopped slugs, coil bolts, lamps)
// `params`: x style, y strength 0..1, z first hit slot (< 0: none), w seed.
// `color`: rgb linear emission, a overall opacity.

struct GlassIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) m0: vec4<f32>,
    @location(3) m1: vec4<f32>,
    @location(4) m2: vec4<f32>,
    @location(5) m3: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) params: vec4<f32>,
};

struct GlassOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) obj: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) color: vec4<f32>,
    @location(4) params: vec4<f32>,
};

// Recent shield hits: xyz the hit direction in the bubble's unit-sphere
// frame, w its age in seconds (< 0: an empty slot). Eight per shield.
struct Hits {
    h: array<vec4<f32>, 64>,
};
@group(1) @binding(0) var<uniform> hits: Hits;

const HITS_PER: i32 = 8;
const RIPPLE_LIFE: f32 = 1.5;
// How fast a ring runs over the bubble, radians per second.
const RIPPLE_SPEED: f32 = 2.4;

// The ripples and hit flashes at unit direction `p` of the shield whose
// slots start at `base`: (ring brightness, flash, outward push).
fn ripples(p: vec3<f32>, base: i32) -> vec3<f32> {
    var ring = 0.0;
    var flash = 0.0;
    var push = 0.0;
    if (base < 0) {
        return vec3<f32>(0.0);
    }
    for (var i = 0; i < HITS_PER; i = i + 1) {
        let h = hits.h[base + i];
        if (h.w < 0.0 || h.w > RIPPLE_LIFE) {
            continue;
        }
        let ang = acos(clamp(dot(p, h.xyz), -1.0, 1.0));
        let front = h.w * RIPPLE_SPEED;
        let fade = exp(-h.w * 1.2) * (1.0 - h.w / RIPPLE_LIFE);
        // The leading ring and a weaker one trailing it.
        let x0 = (ang - front) / 0.08;
        let x1 = (ang - front * 0.6) / 0.05;
        let w = exp(-x0 * x0) + 0.45 * exp(-x1 * x1);
        ring = ring + w * fade;
        // The surface jiggles: a sine wave behind the front, pushed out.
        let behind = clamp(front - ang, 0.0, 1.2);
        push = push + sin((ang - front) * 9.0) * fade * (exp(-x0 * x0) + 0.6 * behind * exp(-behind * 2.0));
        // The struck spot: a white-hot flash, quickly gone.
        flash = flash + exp(-ang * ang / 0.006) * exp(-h.w * 9.0);
    }
    return vec3<f32>(ring, flash, push);
}

@vertex
fn vs_glass(v: GlassIn) -> GlassOut {
    let model = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    let style = u32(v.params.x + 0.5);
    let t = globals.camera.w;
    var p = v.pos;
    if (style == 0u) {
        // Wobble: two slow standing waves, plus the ripples' push.
        let s = v.params.w;
        let wob = 0.012 * sin(p.x * 3.1 + t * 2.3 + s) * sin(p.z * 2.7 - t * 1.7 + s * 2.0) + 0.008 * sin(p.y * 4.0 + t * 3.1 + s * 3.0);
        let r = ripples(normalize(p), i32(v.params.z));
        p = p * (1.0 + wob + 0.05 * r.z + 0.04 * r.y);
    }
    let world = (model * vec4<f32>(p, 1.0)).xyz;
    // The normal of a scaled sphere: by the inverse scale squared.
    let sx = dot(v.m0.xyz, v.m0.xyz);
    let sy = dot(v.m1.xyz, v.m1.xyz);
    let sz = dot(v.m2.xyz, v.m2.xyz);
    let ln = v.normal / max(vec3<f32>(sx, sy, sz), vec3<f32>(1e-6));
    let n = normalize((model * vec4<f32>(ln, 0.0)).xyz);
    var out: GlassOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.obj = v.pos;
    out.normal = n;
    out.color = v.color;
    out.params = v.params;
    return out;
}

@fragment
fn fs_glass(in: GlassOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let style = u32(in.params.x + 0.5);
    let to = in.world - globals.camera.xyz;
    let dist = length(to);
    let dir = to / max(dist, 1e-4);
    let air = air_amount(globals.camera.xyz, dir, dist);
    let v = -dir;
    let t = globals.camera.w;
    let ndv = abs(dot(normalize(in.normal), v));
    let strength = in.params.y;
    // Flat facet normals (derivatives in uniform control flow, before any branch).
    let nf = normalize(cross(dpdx(in.world), dpdy(in.world)));
    // Shells melt away right at the lens.
    let near = smoothstep(0.8, 3.5, dist);
    let k = in.color.a * near * (1.0 - air);
    if (style == 0u) {
        // The low-poly panels catch the light one by one.
        let fdv = abs(dot(nf, v));
        let fres = pow(1.0 - ndv, 2.6);
        let facet = pow(1.0 - fdv, 2.0);
        let cell = floor(nf * 24.0 + 0.5);
        let panel = 0.5 + 0.5 * sin(t * (1.5 + 2.0 * hash13(cell)) + hash13(cell + 7.0) * 6.28);
        let r = ripples(normalize(in.obj), i32(in.params.z));
        // Heat shimmer drifting up the field.
        let shimmer = noise3(in.world * 1.3 + vec3<f32>(0.0, -t * 1.2, 0.0));
        var e = 0.035 + 0.55 * fres + 0.18 * facet * panel + 0.05 * shimmer;
        e = e * (0.55 + 0.45 * strength) + 3.0 * r.x + 6.0 * r.y;
        // The far wall, seen through the near one, is dimmer.
        e = e * select(0.4, 1.0, front);
        // The struck spot runs from white-hot to the field's amber.
        let hot = mix(in.color.rgb, vec3<f32>(1.6, 1.3, 0.9), clamp(r.y, 0.0, 1.0));
        let c = hot * e * k;
        let a = clamp(0.02 + 0.1 * fres + 0.15 * r.x, 0.0, 0.5) * k * strength;
        return vec4<f32>(c, a);
    }
    if (style == 1u) {
        let fres = pow(1.0 - ndv, 2.2);
        let pulse = 0.8 + 0.2 * sin(t * 2.6 + in.params.w * 6.0);
        let c = in.color.rgb * (0.04 + 0.9 * fres) * pulse * select(0.5, 1.0, front) * k;
        return vec4<f32>(c, 0.0);
    }
    if (style == 2u) {
        // obj.y runs 0 at the ground to 1 at the top.
        let y = clamp(in.obj.y, 0.0, 1.0);
        let fall = (1.0 - y) * (1.0 - y);
        let bands = 0.75 + 0.25 * sin(y * 14.0 - t * 3.0 + in.params.w * 6.0);
        let c = in.color.rgb * fall * bands * (0.25 + 0.75 * pow(ndv, 1.5)) * 0.5 * k;
        return vec4<f32>(c, 0.0);
    }
    if (style == 3u) {
        let r = length(in.obj.xz);
        let band = exp(-pow((r - 0.86) / 0.07, 2.0));
        let wave = fract(t * 0.6 + in.params.w);
        let pulse = exp(-pow((r - mix(0.6, 1.05, wave)) / 0.05, 2.0)) * (1.0 - wave);
        let c = in.color.rgb * (band + 0.8 * pulse) * k;
        return vec4<f32>(c, 0.0);
    }
    // Hot glow: a bright core fading to the rim.
    let core = pow(ndv, 2.0);
    let c = in.color.rgb * (0.25 + 1.3 * core) * in.color.a * (1.0 - air * 0.7);
    return vec4<f32>(c, 0.0);
}
