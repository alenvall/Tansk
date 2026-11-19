// Effects sprites and track ruts. Appended to scene.wgsl at load time, so
// they share its globals, lighting ramp, shadows and air.

struct SpriteIn {
    @location(0) pos_size: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) params: vec4<f32>,
    @location(3) axis: vec4<f32>,
};

struct SpriteOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) params: vec4<f32>,
    @location(3) world: vec3<f32>,
};

fn corner(vi: u32) -> vec2<f32> {
    var c = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    return c[vi];
}

@vertex
fn vs_sprite(@builtin(vertex_index) vi: u32, s: SpriteIn) -> SpriteOut {
    let c = corner(vi);
    let kind = u32(s.params.x + 0.5);
    let centre = s.pos_size.xyz;
    var size = s.pos_size.w;
    // Overdraw limit for lit puffs and debris: they fade out inside 3..8 m
    // (see fs_sprite), so one right at the lens is dropped (a zero-area
    // quad) and a near one is kept from covering the screen.
    if (kind == 0u || kind == 3u) {
        let d = length(centre - globals.camera.xyz);
        size = select(min(size, d * 0.7), 0.0, d < 2.5);
    }
    var p = centre;
    var coverage = 1.0;
    if (kind == 2u || kind == 5u) {
        // A streak or cable: a quad stretched along its axis, facing the camera.
        let along = s.axis.xyz;
        let len = length(along);
        let a = select(vec3<f32>(0.0, 1.0, 0.0), along / max(len, 1e-4), len > 1e-4);
        // Clip the segment to in front of the eye: a tracer or cable
        // passing beside or behind the camera otherwise projects through
        // w <= 0 into a streak across the whole screen.
        let eye = globals.camera.xyz;
        let fwd = normalize(cross(globals.cam_up.xyz, globals.cam_right.xyz));
        let half = len * 0.5 + size;
        var e0 = centre - a * half;
        var e1 = centre + a * half;
        let z0 = dot(e0 - eye, fwd);
        let z1 = dot(e1 - eye, fwd);
        let near = 0.6;
        var mid = centre;
        var half_len = half;
        if (max(z0, z1) < near) {
            half_len = 0.0;
            size = 0.0;
        } else {
            if (z0 < near) {
                e0 = mix(e0, e1, (near - z0) / (z1 - z0));
            }
            if (z1 < near) {
                e1 = mix(e1, e0, (near - z1) / (z0 - z1));
            }
            mid = 0.5 * (e0 + e1);
            half_len = 0.5 * length(e1 - e0);
        }
        let view = normalize(mid - eye);
        let side = normalize(cross(a, view) + vec3<f32>(1e-4, 0.0, 0.0));
        // Cables stay at least 1.5 px wide (thinner ones break into dots
        // under FXAA); the fragment fades them by how much they grew.
        var width = size;
        if (kind == 5u) {
            width = select(max(size, 0.75 * globals.cam_right.w * length(mid - eye)), 0.0, size <= 0.0);
            coverage = size / max(width, 1e-5);
        }
        p = mid + a * (c.y * half_len) + side * c.x * width;
    } else {
        let r = s.params.y;
        let rc = vec2<f32>(c.x * cos(r) - c.y * sin(r), c.x * sin(r) + c.y * cos(r));
        p = centre + (globals.cam_right.xyz * rc.x + globals.cam_up.xyz * rc.y) * size;
    }
    var out: SpriteOut;
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    out.uv = c;
    out.color = s.color;
    out.params = s.params;
    if (kind == 5u) {
        out.params.w = coverage;
    }
    out.world = p;
    return out;
}

@fragment
fn fs_sprite(in: SpriteOut) -> @location(0) vec4<f32> {
    let kind = u32(in.params.x + 0.5);
    let r = length(in.uv);
    let seed = in.params.z;
    let to = in.world - globals.camera.xyz;
    let dist = length(to);
    let dir = to / max(dist, 1e-4);
    let air = air_amount(globals.camera.xyz, dir, dist);
    if (kind == 0u || kind == 3u) {
        // Puffy, lit: a lumpy outline and a fake sphere normal.
        let ang = atan2(in.uv.y, in.uv.x);
        var edge = 0.78 + 0.14 * noise2(vec2<f32>(ang * 1.6 + seed * 40.0, seed * 7.0)) + 0.08 * sin(ang * 5.0 + seed * 20.0);
        // Dark smoke (params.w) gets a crisper, more defined silhouette.
        let soft = mix(0.18, 0.07, clamp(in.params.w, 0.0, 1.0));
        var mask = 1.0 - smoothstep(edge - soft, edge, r);
        if (kind == 3u) {
            // Debris: a hard-edged polygon.
            let k = floor((ang + 3.1416) / 1.2566);
            edge = 0.75 + 0.2 * hash12(vec2<f32>(k, seed * 31.0));
            mask = 1.0 - step(edge, r);
        }
        if (mask <= 0.003) {
            discard;
        }
        let back = -dir;
        let z = sqrt(max(1.0 - min(r, 1.0) * min(r, 1.0), 0.0));
        let n = normalize(globals.cam_right.xyz * in.uv.x + globals.cam_up.xyz * in.uv.y + back * (z + 0.35));
        let n_dot_l = dot(n, globals.sun_dir.xyz);
        let hl = 0.5 * n_dot_l + 0.5;
        let shadow = sun_visibility(in.world, globals.sun_dir.xyz, 1.0);
        let lit = globals.sun_color.rgb * warp(hl * hl * mix(0.25, 1.0, shadow)) + ambient(n) + point_lights(in.world, n);
        // Back-lit smoke glows round the edge towards the sun.
        let through = pow(max(dot(dir, globals.sun_dir.xyz), 0.0), 6.0) * (1.0 - z) * 0.8;
        var c = in.color.rgb * lit + globals.sun_color.rgb * in.color.rgb * through;
        c = mix(c, air_colour(dir), air);
        // Soft near fade: smoke, dust and debris right at the lens melt
        // away (3..8 m) instead of filling the screen.
        let a = in.color.a * mask * smoothstep(3.0, 8.0, dist);
        return vec4<f32>(c * a, a);
    }
    if (kind == 1u) {
        // Self-lit blob, additive (alpha 0), with a hot core.
        let mask = pow(max(1.0 - r, 0.0), 1.6);
        let core = pow(max(1.0 - r * 1.6, 0.0), 2.0);
        let c = (in.color.rgb * mask + in.color.rgb * core * 0.8 * in.params.w) * in.color.a * (1.0 - air * 0.7);
        return vec4<f32>(c, 0.0);
    }
    if (kind == 4u) {
        // Shield bubble: a fresnel shell with a hex shimmer, additive.
        if (r > 1.0) {
            discard;
        }
        let rim = pow(r, 6.0) * 0.9 + 0.08;
        let hex = 0.5 + 0.5 * sin((in.uv.x * 9.0 + in.uv.y * 5.2) + globals.camera.w * 3.0 + seed * 20.0) * sin(in.uv.y * 10.4 - globals.camera.w * 2.0);
        let edge = 1.0 - smoothstep(0.96, 1.0, r);
        let c = in.color.rgb * (rim + 0.08 * hex) * edge * in.color.a;
        return vec4<f32>(c, 0.0);
    }
    if (kind == 5u) {
        // Cable: a dark, opaque-ish line.
        let across = 1.0 - smoothstep(0.5, 1.0, abs(in.uv.x));
        let a = across * in.color.a * mix(0.45, 1.0, clamp(in.params.w, 0.0, 1.0));
        let c = mix(in.color.rgb, air_colour(dir), air);
        return vec4<f32>(c * a, a);
    }
    // Streak.
    let across = 1.0 - smoothstep(0.2, 1.0, abs(in.uv.x));
    let ends = 1.0 - smoothstep(0.7, 1.0, abs(in.uv.y));
    let c = in.color.rgb * across * ends * in.color.a * (1.0 - air * 0.7);
    return vec4<f32>(c, 0.0);
}

// ── Track ruts: ground decal strips ─────────────────────────────────────

struct RutIn {
    @location(0) a: vec4<f32>,
    @location(1) b: vec4<f32>,
};

struct RutOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
    @location(2) world: vec3<f32>,
    @location(3) len: f32,
};

@vertex
fn vs_rut(@builtin(vertex_index) vi: u32, r: RutIn) -> RutOut {
    let c = corner(vi);
    let a = r.a.xyz;
    let b = r.b.xyz;
    let d = b - a;
    let side = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), d) + vec3<f32>(1e-5, 0.0, 0.0));
    // Overlap neighbours a little so the strip has no gaps.
    let t = c.y * 0.5 + 0.5;
    let p = mix(a - d * 0.1, b + d * 0.1, t) + side * c.x * r.a.w + vec3<f32>(0.0, 0.05, 0.0);
    var out: RutOut;
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    out.uv = vec2<f32>(c.x, t);
    out.alpha = r.b.w;
    out.world = p;
    out.len = length(d);
    return out;
}

@fragment
fn fs_rut(in: RutOut) -> @location(0) vec4<f32> {
    // Tread pads across the strip, soft edges.
    let pads = smoothstep(0.3, 0.55, abs(fract(in.uv.y * in.len * 3.0) - 0.5) * 2.0);
    let edge = 1.0 - smoothstep(0.6, 1.0, abs(in.uv.x));
    let a = in.alpha * edge * mix(0.65, 1.0, pads) * smoothstep(0.0, 0.1, in.uv.y) * (1.0 - smoothstep(0.9, 1.0, in.uv.y)) ;
    let n = vec3<f32>(0.0, 1.0, 0.0);
    let shadow = sun_visibility(in.world, n, globals.sun_dir.y);
    let hl = 0.5 * globals.sun_dir.y + 0.5;
    let lit = globals.sun_color.rgb * warp(hl * hl * mix(0.1, 1.0, shadow)) + ambient(n);
    let to = in.world - globals.camera.xyz;
    let dist = length(to);
    let dir = to / max(dist, 1e-4);
    var c = SAND * 0.5 * lit;
    c = mix(c, air_colour(dir), air_amount(globals.camera.xyz, dir, dist));
    return vec4<f32>(c * a, a);
}

// ── Blob contact shadows: soft warm darkening under props ───────────────

struct BlobOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_blob(@builtin(vertex_index) vi: u32, @location(0) b: vec4<f32>) -> BlobOut {
    let c = corner(vi);
    let p = b.xyz + vec3<f32>(c.x * b.w, 0.06, c.y * b.w);
    var out: BlobOut;
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    out.uv = c;
    out.world = p;
    return out;
}

@fragment
fn fs_blob(in: BlobOut) -> @location(0) vec4<f32> {
    let r = length(in.uv);
    let a = pow(max(1.0 - r, 0.0), 1.6) * 0.55;
    if (a <= 0.002) {
        discard;
    }
    // Shade, not black: the sand in its own ambient light.
    let n = vec3<f32>(0.0, 1.0, 0.0);
    var c = SAND * 0.55 * ambient(n) * vec3<f32>(0.9, 0.7, 0.62);
    let to = in.world - globals.camera.xyz;
    let dist = length(to);
    let dir = to / max(dist, 1e-4);
    c = mix(c, air_colour(dir), air_amount(globals.camera.xyz, dir, dist));
    return vec4<f32>(c * a, a);
}
