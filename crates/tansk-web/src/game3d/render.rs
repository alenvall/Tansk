//! Drawing the 3D world: sky, terrain, walls, tanks, pickups, projectiles,
//! the zone, shields and effects. The HUD is drawn over it in `hud.rs`.

use tansk3d_sim::game::{BARREL_LENGTH, SHIELD_CENTER_UP, SHIELD_RADIUS, TURRET_PIVOT_UP};
use tansk3d_sim::proto::{color_name, PickupKind, ProjKind, TankView3d, View3d};
use tansk3d_sim::terrain::{self, ARENA_D, ARENA_W, ZONE_RADIUS};
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGlTexture};

use super::gl::{self, add, dir, len, madd, norm, Basis, Blend, Draw, Gl, Mat4, Mesh, Mode, V3};
use crate::assets::Images;

pub const FOG_COLOR: V3 = [0.80, 0.84, 0.88];
const SKIRT: f32 = 400.0;
const GRID: f32 = 8.0;

/// Ground heights on an 8 px grid over the arena, for anything that needs
/// many lookups (terrain::height re-parses the map on every call).
pub struct Heights {
    nx: usize,
    nz: usize,
    h: Vec<f32>,
}

impl Heights {
    pub fn new() -> Heights {
        let nx = (ARENA_W / GRID) as usize + 1;
        let nz = (ARENA_D / GRID) as usize + 1;
        let mut h = Vec::with_capacity(nx * nz);
        for j in 0..nz {
            for i in 0..nx {
                h.push(terrain::height(i as f32 * GRID, j as f32 * GRID));
            }
        }
        Heights { nx, nz, h }
    }

    fn at(&self, i: usize, j: usize) -> f32 {
        self.h[j.min(self.nz - 1) * self.nx + i.min(self.nx - 1)]
    }

    /// Bilinear height at (x, z); outside the arena the rim keeps rising.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let cx = x.clamp(0.0, ARENA_W);
        let cz = z.clamp(0.0, ARENA_D);
        let (fx, fz) = (cx / GRID, cz / GRID);
        let (i, j) = (fx.floor() as usize, fz.floor() as usize);
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let a = self.at(i, j) + (self.at(i + 1, j) - self.at(i, j)) * tx;
        let b = self.at(i, j + 1) + (self.at(i + 1, j + 1) - self.at(i, j + 1)) * tx;
        let out = (x - cx).abs().max((z - cz).abs());
        a + (b - a) * tz + out * 1.5
    }

    pub fn normal(&self, x: f32, z: f32) -> V3 {
        let e = 4.0;
        let hx = self.height(x + e, z) - self.height(x - e, z);
        let hz = self.height(x, z + e) - self.height(x, z - e);
        norm([-hx, 2.0 * e, -hz])
    }
}

pub struct Camera {
    pub eye: V3,
    pub target: V3,
    pub fov: f32,
    pub fog: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum FxKind {
    Explosion,
    Impact,
    Muzzle,
}

pub struct Fx {
    pub kind: FxKind,
    pub pos: V3,
    pub started: f64,
}

impl Fx {
    pub fn duration(&self) -> f64 {
        match self.kind {
            FxKind::Explosion => 1800.0,
            FxKind::Impact => 900.0,
            FxKind::Muzzle => 500.0,
        }
    }
}

/// Your own gun in first person: drawn where the camera looks from.
pub struct OwnGun {
    pub tank: u32,
}

pub struct Frame<'a> {
    pub view: Option<&'a View3d>,
    pub own: Option<OwnGun>,
    pub fx: &'a [Fx],
    pub now: f64,
    /// Where your shell would land, if your weapon arcs.
    pub impact: Option<V3>,
}

pub struct Scene {
    pub gl: Gl,
    cube: Mesh,
    quad: Mesh,
    sphere: Mesh,
    cylinder: Mesh,
    tube: Mesh,
    terrain: Mesh,
    walls: Mesh,
    zone: Mesh,
    blob: Option<WebGlTexture>,
    ring: Option<WebGlTexture>,
    pub vp: Mat4,
    pub eye: V3,
}

fn rgb(c: u8) -> [f32; 3] {
    match color_name(c) {
        "blue" => [0.22, 0.38, 0.95],
        "red" => [0.92, 0.2, 0.18],
        "yellow" => [0.98, 0.84, 0.16],
        "green" => [0.22, 0.78, 0.26],
        "white" => [0.92, 0.92, 0.92],
        "pink" => [0.98, 0.5, 0.74],
        "orange" => [0.98, 0.55, 0.12],
        "purple" => [0.6, 0.3, 0.85],
        "teal" => [0.15, 0.7, 0.7],
        "brown" => [0.55, 0.35, 0.2],
        _ => [0.5, 0.5, 0.5],
    }
}

/// A frame whose up is `n`.
fn basis_up(n: V3) -> Basis {
    let f = norm(gl::cross(n, [0.0, 0.0, 1.0]));
    Basis { f, u: n, r: gl::cross(f, n) }
}

/// Grid lines: coarse out in the skirt, every 8 px over the arena.
fn lines(size: f32) -> Vec<f32> {
    let mut v = Vec::new();
    let mut x = -SKIRT;
    while x < 0.0 {
        v.push(x);
        x += 32.0;
    }
    let mut x = 0.0;
    while x < size {
        v.push(x);
        x += GRID;
    }
    let mut x = size;
    while x <= size + SKIRT + 0.1 {
        v.push(x);
        x += 32.0;
    }
    v
}

fn terrain_mesh(heights: &Heights) -> (Vec<f32>, Vec<u32>) {
    let xs = lines(ARENA_W);
    let zs = lines(ARENA_D);
    let mut v = Vec::with_capacity(xs.len() * zs.len() * 8);
    for &z in &zs {
        for &x in &xs {
            let inside = (0.0..=ARENA_W).contains(&x) && (0.0..=ARENA_D).contains(&z);
            let h = if inside { heights.height(x, z) } else { terrain::height(x, z) };
            let n = if inside { heights.normal(x, z) } else { terrain::normal(x, z) };
            gl::push(&mut v, [x, h, z], n, [0.0, 0.0]);
        }
    }
    let w = xs.len() as u32;
    let mut idx = Vec::new();
    for j in 0..zs.len() as u32 - 1 {
        for i in 0..w - 1 {
            let a = j * w + i;
            let (b, c, d) = (a + 1, a + w + 1, a + w);
            idx.extend_from_slice(&[a, c, b, a, d, c]);
        }
    }
    (v, idx)
}

/// All walls in one mesh, with texture coordinates in world units so the
/// 150x15 wall sprite tiles instead of stretching.
fn walls_mesh() -> Vec<f32> {
    let cube = gl::cube_data();
    let mut v = Vec::new();
    for w in terrain::walls() {
        let size = [w.max[0] - w.min[0], w.max[1] - w.min[1], w.max[2] - w.min[2]];
        let c = [(w.min[0] + w.max[0]) / 2.0, (w.min[1] + w.max[1]) / 2.0, (w.min[2] + w.max[2]) / 2.0];
        let along_x = size[0] >= size[2];
        for k in cube.chunks(8) {
            let p = [c[0] + k[0] * size[0], c[1] + k[1] * size[1], c[2] + k[2] * size[2]];
            let n = [k[3], k[4], k[5]];
            let uv = if n[1].abs() > 0.5 {
                if along_x {
                    [p[0] / 150.0, p[2] / 15.0]
                } else {
                    [p[2] / 150.0, p[0] / 15.0]
                }
            } else if n[0].abs() > 0.5 {
                [p[2] / 150.0, p[1] / 15.0]
            } else {
                [p[0] / 150.0, p[1] / 15.0]
            };
            gl::push(&mut v, p, n, uv);
        }
    }
    v
}

/// The king-of-the-hill disc, following the ground.
fn zone_mesh(heights: &Heights) -> (Vec<f32>, Vec<u32>) {
    let c = terrain::zone_center();
    let (rings, segs) = (8usize, 48usize);
    let mut v = Vec::new();
    for r in 0..=rings {
        for s in 0..segs {
            let rr = ZONE_RADIUS * r as f32 / rings as f32;
            let a = std::f32::consts::TAU * s as f32 / segs as f32;
            let (x, z) = (c[0] + rr * a.cos(), c[2] + rr * a.sin());
            let k = rr / ZONE_RADIUS;
            gl::push(&mut v, [x, heights.height(x, z) + 0.6, z], [0.0, 1.0, 0.0], [0.5 + 0.5 * k * a.cos(), 0.5 + 0.5 * k * a.sin()]);
        }
    }
    let mut idx = Vec::new();
    for r in 0..rings as u32 {
        for s in 0..segs as u32 {
            let s1 = (s + 1) % segs as u32;
            let (a, b, c, d) = (r * segs as u32 + s, r * segs as u32 + s1, (r + 1) * segs as u32 + s1, (r + 1) * segs as u32 + s);
            idx.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    (v, idx)
}

fn paint(size: u32, f: impl Fn(&web_sys::CanvasRenderingContext2d)) -> Option<HtmlCanvasElement> {
    let doc = web_sys::window()?.document()?;
    let c: HtmlCanvasElement = doc.create_element("canvas").ok()?.dyn_into().ok()?;
    c.set_width(size);
    c.set_height(size);
    let ctx: web_sys::CanvasRenderingContext2d = c.get_context("2d").ok()??.dyn_into().ok()?;
    f(&ctx);
    Some(c)
}

impl Scene {
    pub fn new(canvas: HtmlCanvasElement, heights: &Heights) -> Option<Scene> {
        let mut gl = Gl::new(canvas)?;
        let (tv, ti) = terrain_mesh(heights);
        let (zv, zi) = zone_mesh(heights);
        let blob = paint(64, |ctx| {
            if let Ok(g) = ctx.create_radial_gradient(32.0, 32.0, 0.0, 32.0, 32.0, 32.0) {
                let _ = g.add_color_stop(0.0, "rgba(0,0,0,1)");
                let _ = g.add_color_stop(0.6, "rgba(0,0,0,0.6)");
                let _ = g.add_color_stop(1.0, "rgba(0,0,0,0)");
                ctx.set_fill_style_canvas_gradient(&g);
                ctx.fill_rect(0.0, 0.0, 64.0, 64.0);
            }
        })
        .and_then(|c| gl.canvas_texture("__blob", &c));
        let ring = paint(64, |ctx| {
            ctx.set_stroke_style_str("rgba(255,255,255,1)");
            ctx.set_line_width(5.0);
            ctx.begin_path();
            let _ = ctx.arc(32.0, 32.0, 27.0, 0.0, std::f64::consts::TAU);
            ctx.stroke();
            ctx.set_line_width(3.0);
            ctx.begin_path();
            ctx.move_to(32.0, 14.0);
            ctx.line_to(32.0, 50.0);
            ctx.move_to(14.0, 32.0);
            ctx.line_to(50.0, 32.0);
            ctx.stroke();
        })
        .and_then(|c| gl.canvas_texture("__ring", &c));
        Some(Scene {
            cube: gl.mesh(&gl::cube_data(), None)?,
            quad: gl.mesh(&gl::quad_data(), None)?,
            sphere: gl.mesh(&gl::sphere_data(10, 16), None)?,
            cylinder: gl.mesh(&gl::cylinder_data(12), None)?,
            tube: gl.mesh(&gl::tube_data(48), None)?,
            terrain: gl.mesh(&tv, Some(&ti))?,
            walls: gl.mesh(&walls_mesh(), None)?,
            zone: gl.mesh(&zv, Some(&zi))?,
            blob,
            ring,
            vp: [0.0; 16],
            eye: [0.0; 3],
            gl,
        })
    }

    /// A world point in HUD pixels (1024x768) and its distance, if in front of the camera.
    pub fn project(&self, p: V3) -> Option<(f64, f64, f32)> {
        let m = &self.vp;
        let x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
        let y = m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13];
        let w = m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15];
        if w < 1.0 {
            return None;
        }
        Some((((x / w + 1.0) * 512.0) as f64, ((1.0 - y / w) * 384.0) as f64, len(gl::sub(p, self.eye))))
    }

    fn mesh_draw(&mut self, images: &Images, which: M, m: &Mat4, d: Draw) {
        let mesh = match which {
            M::Cube => &self.cube,
            M::Quad => &self.quad,
            M::Sphere => &self.sphere,
            M::Cylinder => &self.cylinder,
            M::Tube => &self.tube,
        };
        self.gl.draw(images, mesh, m, d);
    }

    fn sphere(&mut self, images: &Images, pos: V3, r: f32, d: Draw) {
        self.mesh_draw(images, M::Sphere, &gl::identity_at(pos, [r * 2.0; 3]), d);
    }

    pub fn draw(&mut self, images: &Images, cam: &Camera, f: &Frame) {
        let aspect = 1024.0 / 768.0;
        let proj = gl::perspective(cam.fov, aspect, 1.5, 6000.0);
        let view = gl::look_at(cam.eye, cam.target, [0.0, 1.0, 0.0]);
        self.vp = gl::mul(&proj, &view);
        self.eye = cam.eye;
        let light = norm([-0.45, 0.85, -0.35]);
        self.gl.begin(&self.vp, cam.eye, light, FOG_COLOR, cam.fog);
        let t_s = (f.now / 1000.0) as f32;

        // Sky: a big dome around the eye.
        self.gl.solid(false);
        self.sphere(images, cam.eye, 5000.0, Draw { mode: Mode::Sky, ..Draw::unlit([1.0; 4]) });
        self.gl.solid(true);

        // Terrain and walls.
        let ground = self.gl.texture(images, "map", true);
        let tdraw = Draw { color: if ground.is_some() { [1.0; 4] } else { [0.86, 0.8, 0.68, 1.0] }, tex: None, mode: Mode::Terrain, uv_scale: [1.0 / 300.0, 1.0 / 300.0] };
        self.gl.draw_with(&self.terrain, &gl::identity_at([0.0; 3], [1.0; 3]), tdraw, ground.as_ref());
        let wall = self.gl.texture(images, "horizontal_wall", true);
        let wdraw = Draw { color: if wall.is_some() { [1.0; 4] } else { [0.35, 0.35, 0.35, 1.0] }, ..Draw::lit([1.0; 3]) };
        self.gl.draw_with(&self.walls, &gl::identity_at([0.0; 3], [1.0; 3]), wdraw, wall.as_ref());

        // Decals on the ground.
        let g = self.gl.gl.clone();
        g.enable(web_sys::WebGl2RenderingContext::POLYGON_OFFSET_FILL);
        g.polygon_offset(-1.0, -4.0);
        self.gl.solid(false);
        for (sx, sz, rot) in SPAWNS.with(|s| s.clone()) {
            let y = terrain_y(sx, sz);
            self.mesh_draw(images, M::Quad, &Basis::yaw(rot).model([sx, y + 0.3, sz], [50.0, 1.0, 65.0]), Draw { color: [1.0, 1.0, 1.0, 0.8], ..Draw::tex("tank_spawnpoint", Mode::Lit) });
        }
        let zone = f.view.and_then(|v| v.zone);
        if zone.is_some() {
            let pulse = 0.55 + 0.15 * (t_s * 3.0).sin();
            self.gl.draw_with(&self.zone, &gl::identity_at([0.0; 3], [1.0; 3]), Draw::unlit([1.0, 0.85, 0.25, pulse * 0.5]), None);
            self.gl.draw(images, &self.zone, &gl::identity_at([0.0; 3], [1.0; 3]), Draw { color: [1.0, 1.0, 1.0, 0.7], ..Draw::tex("king_of_the_hill_zone", Mode::Unlit) });
        }
        if let Some(v) = f.view {
            let blob = self.blob.clone();
            for t in &v.tanks {
                let b = Basis::yaw(t.yaw).pitched(t.hull_pitch).rolled(t.hull_roll);
                let m = b.model(b.at(t.pos, [4.0, 0.8, 4.0]), [80.0, 1.0, 62.0]);
                self.gl.draw_with(&self.quad, &m, Draw::unlit([1.0, 1.0, 1.0, 0.5]), blob.as_ref());
            }
            for p in &v.pickups {
                let m = gl::identity_at([p.pos[0], p.pos[1] + 0.8, p.pos[2]], [36.0, 1.0, 36.0]);
                self.gl.draw_with(&self.quad, &m, Draw::unlit([1.0, 1.0, 1.0, 0.35]), blob.as_ref());
            }
        }
        if let Some(p) = f.impact {
            let n = HEIGHTS.with(|h| h.borrow().as_ref().map(|h| h.normal(p[0], p[2]))).unwrap_or([0.0, 1.0, 0.0]);
            let ring = self.ring.clone();
            let s = 26.0 + 3.0 * (t_s * 6.0).sin();
            self.gl.draw_with(&self.quad, &basis_up(n).model(madd(p, n, 0.5), [s, 1.0, s]), Draw::unlit([1.0, 0.25, 0.15, 0.85]), ring.as_ref());
        }
        self.gl.solid(true);
        g.disable(web_sys::WebGl2RenderingContext::POLYGON_OFFSET_FILL);

        let Some(v) = f.view else {
            self.gl.end();
            return;
        };

        // Tanks.
        for t in &v.tanks {
            self.tank(images, t);
        }

        // Pickups: spinning crates wearing the original icons.
        for p in &v.pickups {
            let size = if matches!(p.kind, PickupKind::Weapon(_)) { 26.0 } else { 22.0 };
            let bob = 4.0 * (t_s * 2.5 + p.id as f32).sin();
            let m = Basis::yaw(t_s * 70.0 + p.id as f32 * 13.0).model([p.pos[0], p.pos[1] + 24.0 + bob, p.pos[2]], [size; 3]);
            self.mesh_draw(images, M::Cube, &m, Draw::tex(p.kind.sprite(), Mode::Lit));
        }

        // Glowing things: projectiles, the zone's beam, effects.
        self.gl.solid(false);
        self.gl.blend(Blend::Add);
        for p in &v.projectiles {
            let speed = len(p.vel);
            let d = if speed > 1e-5 { norm(p.vel) } else { [1.0, 0.0, 0.0] };
            let b = Basis::along(d);
            let (core, glow, trail): (V3, [f32; 4], usize) = match p.kind {
                ProjKind::Default => ([12.0, 5.0, 5.0], [1.0, 0.8, 0.35, 1.0], 9),
                ProjKind::Bounce => ([10.0, 6.0, 6.0], [1.0, 0.45, 0.1, 1.0], 7),
                ProjKind::Shotgun => ([6.0, 3.0, 3.0], [1.0, 0.9, 0.6, 0.9], 4),
                ProjKind::Slowspeedy => ([14.0, 6.0, 6.0], [0.35, 1.0, 0.9, 1.0], 5),
                ProjKind::Flame => {
                    let k = ((p.id * 37) % 11) as f32 / 11.0;
                    self.sphere(images, p.pos, 7.0 + 5.0 * k, Draw::soft([1.0, 0.35 + 0.35 * k, 0.05, 0.9]));
                    continue;
                }
                ProjKind::Shockwave => {
                    self.sphere(images, p.pos, 14.0 + 2.0 * (t_s * 20.0).sin(), Draw::glow([0.35, 0.6, 1.0, 1.0]));
                    self.sphere(images, p.pos, 7.0, Draw::unlit([0.8, 0.9, 1.0, 1.0]));
                    continue;
                }
                ProjKind::ShockwaveSecondary => {
                    self.sphere(images, p.pos, 5.0, Draw::glow([0.5, 0.95, 1.0, 1.0]));
                    continue;
                }
            };
            self.mesh_draw(images, M::Sphere, &b.model(p.pos, core), Draw::unlit([1.0, 1.0, 0.9, 1.0]));
            self.mesh_draw(images, M::Sphere, &b.model(p.pos, [core[0] * 1.8, core[1] * 2.4, core[2] * 2.4]), Draw::soft(glow));
            for i in 1..=trail {
                let k = i as f32 / trail as f32;
                let r = core[1] * 0.55 * (1.0 - k) + 0.6;
                let c = [glow[0], glow[1], glow[2], 0.55 * (1.0 - k)];
                self.sphere(images, madd(p.pos, d, -(i as f32) * 6.0), r, Draw::unlit(c));
            }
        }
        if let Some(z) = zone {
            let c = terrain::zone_center();
            let base = [z[0], c[1] - 4.0, z[2]];
            let h = 70.0 + 6.0 * (t_s * 2.0).sin();
            self.mesh_draw(images, M::Tube, &gl::identity_at(base, [ZONE_RADIUS * 2.0, h, ZONE_RADIUS * 2.0]), Draw { mode: Mode::Beam, ..Draw::unlit([1.0, 0.8, 0.2, 0.45]) });
        }
        self.effects(images, f.fx, f.now, true);
        self.gl.blend(Blend::Alpha);
        self.effects(images, f.fx, f.now, false);

        // Shields.
        let own_id = f.own.as_ref().map(|o| o.tank);
        for t in &v.tanks {
            if Some(t.id) == own_id {
                continue;
            }
            if let Some(s) = t.shield {
                let strong = s > 100.0;
                let a = if strong { 0.45 + 0.15 * (t_s * 8.0).sin() } else { 0.55 };
                let color = if strong { [1.0, 0.95, 0.5, a] } else { [0.45, 0.85, 1.0, a] };
                let pos = add(t.pos, [0.0, SHIELD_CENTER_UP, 0.0]);
                self.sphere(images, pos, SHIELD_RADIUS, Draw::glow(color));
            }
        }
        self.gl.solid(true);

        // Your own gun, last, over everything near.
        if let Some(own) = &f.own {
            if let Some(t) = v.tanks.iter().find(|t| t.id == own.tank) {
                self.gl.gl.clear(web_sys::WebGl2RenderingContext::DEPTH_BUFFER_BIT);
                self.gl.set_fog(0.0);
                self.barrel(images, t, 12.0, 5.0);
                self.gl.set_fog(cam.fog);
            }
        }
        self.gl.end();
    }

    fn effects(&mut self, images: &Images, fx: &[Fx], now: f64, additive: bool) {
        for (i, e) in fx.iter().enumerate() {
            let age = ((now - e.started) / 1000.0) as f32;
            let seed = (i as f32 * 2.399 + e.pos[0] * 0.13 + e.pos[2] * 0.07).fract() * std::f32::consts::TAU;
            match (e.kind, additive) {
                (FxKind::Explosion, true) => {
                    let k = (age / 0.6).min(1.0);
                    if age < 0.6 {
                        self.sphere(images, add(e.pos, [0.0, 12.0 + 20.0 * k, 0.0]), 18.0 + 55.0 * k.sqrt(), Draw::soft([1.0, 0.75 - 0.45 * k, 0.25 - 0.2 * k, 1.0 - k]));
                    }
                    if age < 0.15 {
                        self.sphere(images, add(e.pos, [0.0, 14.0, 0.0]), 30.0 + 120.0 * age, Draw::unlit([1.0, 0.95, 0.8, 1.0 - age / 0.15]));
                    }
                    for s in 0..6 {
                        let a = seed + s as f32 * 1.05;
                        let kk = (age / 1.0).min(1.0);
                        if kk < 1.0 {
                            let p = add(e.pos, [a.cos() * 50.0 * kk, 10.0 + 60.0 * kk - 80.0 * kk * kk, a.sin() * 50.0 * kk]);
                            self.sphere(images, p, 3.0 * (1.0 - kk) + 1.0, Draw::unlit([1.0, 0.7, 0.3, 1.0 - kk]));
                        }
                    }
                }
                (FxKind::Explosion, false) => {
                    let k = (age / 1.8).min(1.0);
                    for s in 0..7 {
                        let a = seed + s as f32 * 0.9;
                        let rad = 20.0 * (1.0 + k);
                        let p = add(e.pos, [a.cos() * rad, 18.0 + 70.0 * k + s as f32 * 5.0, a.sin() * rad]);
                        let sh = 0.18 + 0.05 * (s % 3) as f32;
                        self.sphere(images, p, 16.0 + 34.0 * k, Draw { color: [sh, sh, sh * 0.95, 0.55 * (1.0 - k) * (age / 0.15).min(1.0)], ..Draw::soft([1.0; 4]) });
                    }
                }
                (FxKind::Impact, true) => {
                    if age < 0.12 {
                        self.sphere(images, e.pos, 10.0, Draw::soft([1.0, 0.8, 0.4, 1.0 - age / 0.12]));
                    }
                }
                (FxKind::Impact, false) => {
                    let k = (age / 0.9).min(1.0);
                    for s in 0..4 {
                        let a = seed + s as f32 * 1.6;
                        let p = add(e.pos, [a.cos() * 8.0 * (1.0 + 2.0 * k), 4.0 + 18.0 * k + s as f32 * 2.0, a.sin() * 8.0 * (1.0 + 2.0 * k)]);
                        self.sphere(images, p, 6.0 + 14.0 * k, Draw { color: [0.62, 0.53, 0.4, 0.6 * (1.0 - k)], ..Draw::soft([1.0; 4]) });
                    }
                }
                (FxKind::Muzzle, true) => {
                    if age < 0.1 {
                        let k = age / 0.1;
                        // Your own flash is right in front of the eye: keep it small.
                        let near = (len(gl::sub(e.pos, self.eye)) / 110.0).clamp(0.35, 1.0);
                        self.sphere(images, e.pos, (7.0 + 10.0 * k) * near, Draw::soft([1.0, 0.85, 0.4, 1.0 - k]));
                        self.sphere(images, e.pos, 4.0 * near, Draw::unlit([1.0, 1.0, 0.85, 1.0 - k]));
                    }
                }
                (FxKind::Muzzle, false) => {
                    let k = (age / 0.5).min(1.0);
                    self.sphere(images, add(e.pos, [0.0, 10.0 * k, 0.0]), 5.0 + 10.0 * k, Draw { color: [0.6, 0.6, 0.6, 0.35 * (1.0 - k)], ..Draw::soft([1.0; 4]) });
                }
            }
        }
    }

    /// The turret's frame and the barrel's start.
    fn turret(t: &TankView3d) -> (V3, Basis) {
        let pivot = add(t.pos, [0.0, TURRET_PIVOT_UP, 0.0]);
        (pivot, Basis::yaw(t.turret_yaw))
    }

    /// The barrel from `from` along it to the muzzle, `thick` across.
    fn barrel(&mut self, images: &Images, t: &TankView3d, from: f32, thick: f32) {
        let (pivot, _) = Self::turret(t);
        let b = Basis::yaw(t.turret_yaw).pitched(t.barrel_pitch);
        let start = b.at(pivot, [from, 0.0, 0.0]);
        let c = [0.21, 0.21, 0.2];
        self.mesh_draw(images, M::Cylinder, &b.model(start, [BARREL_LENGTH - from, thick, thick]), Draw::lit(c));
        let k = thick / 6.0;
        self.mesh_draw(images, M::Cube, &b.model(b.at(pivot, [BARREL_LENGTH - 3.0, 0.0, 0.0]), [6.0, 8.0 * k, 8.0 * k]), Draw::lit([0.16, 0.16, 0.15]));
    }

    fn tank(&mut self, images: &Images, t: &TankView3d) {
        let base = rgb(t.color);
        let dark = [base[0] * 0.55, base[1] * 0.55, base[2] * 0.55];
        let hull = Basis::yaw(t.yaw).pitched(t.hull_pitch).rolled(t.hull_roll);
        let p = t.pos;
        let m = |local: V3, size: V3| hull.model(hull.at(p, local), size);
        for side in [-1.0f32, 1.0] {
            self.mesh_draw(images, M::Cube, &m([0.0, 8.0, side * 19.0], [64.0, 16.0, 12.0]), Draw::lit([0.16, 0.16, 0.15]));
            for k in -2..=2 {
                self.mesh_draw(images, M::Cube, &m([k as f32 * 12.0, 8.0, side * 25.3], [8.0, 8.0, 1.2]), Draw::lit([0.35, 0.35, 0.33]));
            }
            // Fenders over the tracks, in the player colour.
            self.mesh_draw(images, M::Cube, &m([0.0, 16.5, side * 19.0], [62.0, 2.0, 13.0]), Draw::lit(dark));
        }
        self.mesh_draw(images, M::Cube, &m([0.0, 17.0, 0.0], [54.0, 12.0, 28.0]), Draw::lit(base));
        self.mesh_draw(images, M::Cube, &m([26.0, 14.0, 0.0], [8.0, 8.0, 28.0]), Draw::lit(dark));
        self.mesh_draw(images, M::Cube, &m([-24.0, 21.0, 0.0], [8.0, 6.0, 24.0]), Draw::lit([0.25, 0.25, 0.24]));

        let (pivot, tb) = Self::turret(t);
        self.mesh_draw(images, M::Cube, &tb.model(tb.at(pivot, [-1.0, -3.0, 0.0]), [26.0, 10.0, 22.0]), Draw::lit(dark));
        self.mesh_draw(images, M::Cube, &tb.model(tb.at(pivot, [-4.0, 3.0, 0.0]), [12.0, 3.0, 12.0]), Draw::lit(base));
        self.barrel(images, t, 4.0, 6.0);
    }
}

#[derive(Clone, Copy)]
enum M {
    Cube,
    Quad,
    Sphere,
    Cylinder,
    Tube,
}

thread_local! {
    static SPAWNS: Vec<(f32, f32, f32)> = terrain::spawn_points();
    /// The shared height grid (set once at start-up).
    pub static HEIGHTS: std::cell::RefCell<Option<std::rc::Rc<Heights>>> = const { std::cell::RefCell::new(None) };
}

/// Ground height from the shared grid (falls back to the exact function).
pub fn terrain_y(x: f32, z: f32) -> f32 {
    HEIGHTS.with(|h| h.borrow().as_ref().map(|h| h.height(x, z))).unwrap_or_else(|| terrain::height(x, z))
}

/// The muzzle of a tank, as the rules place it.
pub fn muzzle(t: &TankView3d) -> V3 {
    madd(add(t.pos, [0.0, TURRET_PIVOT_UP, 0.0]), dir(t.turret_yaw, t.barrel_pitch), BARREL_LENGTH)
}
