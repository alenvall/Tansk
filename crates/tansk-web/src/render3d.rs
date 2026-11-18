//! The 3D view: the same game drawn with WebGL2 under a tilted camera.
//! World coordinates are the game's: x right, z = the game's y (down the
//! screen), y up. One unit is one pixel of the original 1024x768 arena.

use std::collections::HashMap;

use tansk_sim::game::{SHIELD_RADIUS, TURRET_LENGTH};
use tansk_sim::map::{self, MapEntity, HORIZONTAL_WALL};
use tansk_sim::proto::{color_name, GameView, PickupKind, ProjKind};
use tansk_sim::V2;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, HtmlImageElement, WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram, WebGlTexture,
    WebGlUniformLocation, WebGlVertexArrayObject,
};

use crate::assets::Images;
use crate::render::Explosion;

type Mat4 = [f32; 16];
type V3 = [f32; 3];

const FOV_Y: f32 = 40.0;
const EYE: V3 = [512.0, 1000.0, 384.0 + 700.0];
const TARGET: V3 = [512.0, 0.0, 412.0];
const WALL_HEIGHT: f32 = 34.0;
const BARREL_Y: f32 = 31.0;

const VS: &str = r#"#version 300 es
layout(location=0) in vec3 a_pos;
layout(location=1) in vec3 a_nrm;
layout(location=2) in vec2 a_uv;
uniform mat4 u_vp;
uniform mat4 u_model;
out vec3 v_nrm;
out vec2 v_uv;
void main() {
    v_nrm = mat3(u_model) * a_nrm;
    v_uv = a_uv;
    gl_Position = u_vp * u_model * vec4(a_pos, 1.0);
}"#;

const FS: &str = r#"#version 300 es
precision mediump float;
in vec3 v_nrm;
in vec2 v_uv;
uniform vec4 u_color;
uniform sampler2D u_tex;
uniform int u_use_tex;
uniform int u_lit;
uniform vec3 u_light;
out vec4 o;
void main() {
    vec4 c = u_color;
    if (u_use_tex == 1) {
        c *= texture(u_tex, v_uv);
        if (c.a < 0.03) discard;
    }
    if (u_lit == 1) {
        float d = max(dot(normalize(v_nrm), u_light), 0.0);
        c.rgb *= 0.42 + 0.68 * d;
    }
    o = c;
}"#;

struct Mesh {
    vao: WebGlVertexArrayObject,
    _vbo: WebGlBuffer,
    count: i32,
}

pub struct Renderer3d {
    pub canvas: HtmlCanvasElement,
    gl: GL,
    prog: WebGlProgram,
    u_vp: WebGlUniformLocation,
    u_model: WebGlUniformLocation,
    u_color: WebGlUniformLocation,
    u_use_tex: WebGlUniformLocation,
    u_lit: WebGlUniformLocation,
    u_light: WebGlUniformLocation,
    cube: Mesh,
    quad: Mesh,
    sphere: Mesh,
    cylinder: Mesh,
    textures: HashMap<String, WebGlTexture>,
    blob: Option<WebGlTexture>,
    vp: Mat4,
    aspect: f32,
}

// ------------------------------------------------------------------ math

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn perspective(fovy_deg: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fovy_deg.to_radians() / 2.0).tan();
    [
        f / aspect, 0.0, 0.0, 0.0,
        0.0, f, 0.0, 0.0,
        0.0, 0.0, (far + near) / (near - far), -1.0,
        0.0, 0.0, 2.0 * far * near / (near - far), 0.0,
    ]
}

fn look_at(eye: V3, center: V3, up: V3) -> Mat4 {
    let f = norm(sub(center, eye));
    let s = norm(cross(f, up));
    let u = cross(s, f);
    [
        s[0], u[0], -f[0], 0.0,
        s[1], u[1], -f[1], 0.0,
        s[2], u[2], -f[2], 0.0,
        -dot(s, eye), -dot(u, eye), dot(f, eye), 1.0,
    ]
}

fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut r = [0.0; 16];
    for c in 0..4 {
        for row in 0..4 {
            r[c * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[c * 4 + k]).sum();
        }
    }
    r
}

/// Translate * rotate in the ground plane (by the game's angle) * scale.
fn model(pos: V3, theta_deg: f32, scale: V3) -> Mat4 {
    let (s, c) = theta_deg.to_radians().sin_cos();
    [
        c * scale[0], 0.0, s * scale[0], 0.0,
        0.0, scale[1], 0.0, 0.0,
        -s * scale[2], 0.0, c * scale[2], 0.0,
        pos[0], pos[1], pos[2], 1.0,
    ]
}

/// A point `local` in a model's frame (at `pos`, turned by `theta`).
fn offset(pos: V3, theta_deg: f32, local: V3) -> V3 {
    let (s, c) = theta_deg.to_radians().sin_cos();
    [pos[0] + local[0] * c - local[2] * s, pos[1] + local[1], pos[2] + local[0] * s + local[2] * c]
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

// ---------------------------------------------------------------- meshes

fn push(v: &mut Vec<f32>, p: V3, n: V3, uv: [f32; 2]) {
    v.extend_from_slice(&[p[0], p[1], p[2], n[0], n[1], n[2], uv[0], uv[1]]);
}

/// A unit cube centred on the origin.
fn cube_data() -> Vec<f32> {
    let mut v = Vec::new();
    // (normal, u axis, v axis)
    let faces: [(V3, V3, V3); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, -1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
        ([0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
    ];
    for (n, u, w) in faces {
        let corner = |a: f32, b: f32| -> (V3, [f32; 2]) {
            (
                [
                    n[0] * 0.5 + u[0] * a + w[0] * b,
                    n[1] * 0.5 + u[1] * a + w[1] * b,
                    n[2] * 0.5 + u[2] * a + w[2] * b,
                ],
                [a + 0.5, b + 0.5],
            )
        };
        let q = [corner(-0.5, -0.5), corner(0.5, -0.5), corner(0.5, 0.5), corner(-0.5, 0.5)];
        for i in [0, 1, 2, 0, 2, 3] {
            push(&mut v, q[i].0, n, q[i].1);
        }
    }
    v
}

/// A unit square on the ground, facing up; u along x, v along z.
fn quad_data() -> Vec<f32> {
    let mut v = Vec::new();
    let n = [0.0, 1.0, 0.0];
    let q = [([-0.5, 0.0, -0.5], [0.0, 0.0]), ([0.5, 0.0, -0.5], [1.0, 0.0]), ([0.5, 0.0, 0.5], [1.0, 1.0]), ([-0.5, 0.0, 0.5], [0.0, 1.0])];
    for i in [0, 2, 1, 0, 3, 2] {
        push(&mut v, q[i].0, n, q[i].1);
    }
    v
}

fn sphere_data(rings: usize, segs: usize) -> Vec<f32> {
    let mut v = Vec::new();
    let p = |r: usize, s: usize| -> V3 {
        let th = std::f32::consts::PI * r as f32 / rings as f32;
        let ph = std::f32::consts::TAU * s as f32 / segs as f32;
        [th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin()]
    };
    for r in 0..rings {
        for s in 0..segs {
            let a = p(r, s);
            let b = p(r + 1, s);
            let c = p(r + 1, s + 1);
            let d = p(r, s + 1);
            for q in [a, b, c, a, c, d] {
                push(&mut v, [q[0] * 0.5, q[1] * 0.5, q[2] * 0.5], q, [0.0, 0.0]);
            }
        }
    }
    v
}

/// A unit cylinder along x, from x = 0 to 1, radius 0.5.
fn cylinder_data(segs: usize) -> Vec<f32> {
    let mut v = Vec::new();
    for s in 0..segs {
        let a0 = std::f32::consts::TAU * s as f32 / segs as f32;
        let a1 = std::f32::consts::TAU * (s + 1) as f32 / segs as f32;
        let (y0, z0, y1, z1) = (a0.cos() * 0.5, a0.sin() * 0.5, a1.cos() * 0.5, a1.sin() * 0.5);
        let n0 = [0.0, y0 * 2.0, z0 * 2.0];
        let n1 = [0.0, y1 * 2.0, z1 * 2.0];
        push(&mut v, [0.0, y0, z0], n0, [0.0, 0.0]);
        push(&mut v, [1.0, y0, z0], n0, [0.0, 0.0]);
        push(&mut v, [1.0, y1, z1], n1, [0.0, 0.0]);
        push(&mut v, [0.0, y0, z0], n0, [0.0, 0.0]);
        push(&mut v, [1.0, y1, z1], n1, [0.0, 0.0]);
        push(&mut v, [0.0, y1, z1], n1, [0.0, 0.0]);
        // The muzzle end cap.
        push(&mut v, [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0]);
        push(&mut v, [1.0, y0, z0], [1.0, 0.0, 0.0], [0.0, 0.0]);
        push(&mut v, [1.0, y1, z1], [1.0, 0.0, 0.0], [0.0, 0.0]);
    }
    v
}

#[derive(Clone, Copy)]
struct Draw<'a> {
    color: [f32; 4],
    tex: Option<&'a str>,
    lit: bool,
}

impl Renderer3d {
    pub fn new(canvas: HtmlCanvasElement) -> Option<Self> {
        let gl: GL = canvas.get_context("webgl2").ok()??.dyn_into().ok()?;
        let compile = |kind, src: &str| {
            let s = gl.create_shader(kind)?;
            gl.shader_source(&s, src);
            gl.compile_shader(&s);
            if !gl.get_shader_parameter(&s, GL::COMPILE_STATUS).as_bool().unwrap_or(false) {
                web_sys::console::error_1(&gl.get_shader_info_log(&s).unwrap_or_default().into());
                return None;
            }
            Some(s)
        };
        let vs = compile(GL::VERTEX_SHADER, VS)?;
        let fs = compile(GL::FRAGMENT_SHADER, FS)?;
        let prog = gl.create_program()?;
        gl.attach_shader(&prog, &vs);
        gl.attach_shader(&prog, &fs);
        gl.link_program(&prog);
        if !gl.get_program_parameter(&prog, GL::LINK_STATUS).as_bool().unwrap_or(false) {
            web_sys::console::error_1(&gl.get_program_info_log(&prog).unwrap_or_default().into());
            return None;
        }
        let u = |n: &str| gl.get_uniform_location(&prog, n);
        let mesh = |data: Vec<f32>| -> Option<Mesh> {
            let vao = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(&vao));
            let vbo = gl.create_buffer()?;
            gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vbo));
            let arr = js_sys::Float32Array::from(data.as_slice());
            gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &arr, GL::STATIC_DRAW);
            let stride = 8 * 4;
            for (loc, size, off) in [(0, 3, 0), (1, 3, 12), (2, 2, 24)] {
                gl.enable_vertex_attrib_array(loc);
                gl.vertex_attrib_pointer_with_i32(loc, size, GL::FLOAT, false, stride, off);
            }
            gl.bind_vertex_array(None);
            Some(Mesh { vao, _vbo: vbo, count: (data.len() / 8) as i32 })
        };
        let r = Renderer3d {
            u_vp: u("u_vp")?,
            u_model: u("u_model")?,
            u_color: u("u_color")?,
            u_use_tex: u("u_use_tex")?,
            u_lit: u("u_lit")?,
            u_light: u("u_light")?,
            cube: mesh(cube_data())?,
            quad: mesh(quad_data())?,
            sphere: mesh(sphere_data(12, 20))?,
            cylinder: mesh(cylinder_data(12))?,
            textures: HashMap::new(),
            blob: None,
            vp: [0.0; 16],
            aspect: 4.0 / 3.0,
            prog,
            gl,
            canvas,
        };
        Some(r)
    }

    /// Where the mouse (canvas pixels, 1024x768) points on the ground.
    pub fn unproject(&self, m: V2) -> V2 {
        let f = norm(sub(TARGET, EYE));
        let s = norm(cross(f, [0.0, 1.0, 0.0]));
        let u = cross(s, f);
        let t = (FOV_Y.to_radians() / 2.0).tan();
        let nx = m.x / 512.0 - 1.0;
        let ny = 1.0 - m.y / 384.0;
        let d: V3 = [
            f[0] + s[0] * nx * t * self.aspect + u[0] * ny * t,
            f[1] + s[1] * nx * t * self.aspect + u[1] * ny * t,
            f[2] + s[2] * nx * t * self.aspect + u[2] * ny * t,
        ];
        if d[1] >= -1e-4 {
            return V2::new(m.x, m.y);
        }
        let k = -EYE[1] / d[1];
        V2::new(EYE[0] + d[0] * k, EYE[2] + d[2] * k)
    }

    /// A world point in canvas pixels (1024x768).
    pub fn project(&self, p: V3) -> V2 {
        let m = &self.vp;
        let x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
        let y = m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13];
        let w = m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15];
        V2::new((x / w + 1.0) * 512.0, (1.0 - y / w) * 384.0)
    }

    fn texture(&mut self, images: &Images, name: &str) -> Option<WebGlTexture> {
        if let Some(t) = self.textures.get(name) {
            return Some(t.clone());
        }
        let img: HtmlImageElement = images.get(name)?;
        let gl = &self.gl;
        let t = gl.create_texture()?;
        gl.bind_texture(GL::TEXTURE_2D, Some(&t));
        gl.tex_image_2d_with_u32_and_u32_and_html_image_element(GL::TEXTURE_2D, 0, GL::RGBA as i32, GL::RGBA, GL::UNSIGNED_BYTE, &img).ok()?;
        gl.generate_mipmap(GL::TEXTURE_2D);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::LINEAR_MIPMAP_LINEAR as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::LINEAR as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);
        self.textures.insert(name.to_string(), t.clone());
        Some(t)
    }

    /// A soft round shadow, painted once on a small canvas.
    fn blob_texture(&mut self) -> Option<WebGlTexture> {
        if let Some(t) = &self.blob {
            return Some(t.clone());
        }
        let doc = web_sys::window()?.document()?;
        let c: HtmlCanvasElement = doc.create_element("canvas").ok()?.dyn_into().ok()?;
        c.set_width(64);
        c.set_height(64);
        let ctx: web_sys::CanvasRenderingContext2d = c.get_context("2d").ok()??.dyn_into().ok()?;
        let g = ctx.create_radial_gradient(32.0, 32.0, 0.0, 32.0, 32.0, 32.0).ok()?;
        let _ = g.add_color_stop(0.0, "rgba(0,0,0,1)");
        let _ = g.add_color_stop(0.6, "rgba(0,0,0,0.6)");
        let _ = g.add_color_stop(1.0, "rgba(0,0,0,0)");
        ctx.set_fill_style_canvas_gradient(&g);
        ctx.fill_rect(0.0, 0.0, 64.0, 64.0);
        let gl = &self.gl;
        let t = gl.create_texture()?;
        gl.bind_texture(GL::TEXTURE_2D, Some(&t));
        gl.tex_image_2d_with_u32_and_u32_and_html_canvas_element(GL::TEXTURE_2D, 0, GL::RGBA as i32, GL::RGBA, GL::UNSIGNED_BYTE, &c).ok()?;
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::LINEAR as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);
        self.blob = Some(t.clone());
        Some(t)
    }

    fn draw_mesh(&mut self, images: &Images, mesh: MeshKind, m: &Mat4, d: Draw) {
        let tex = match d.tex {
            Some("__blob") => self.blob_texture(),
            Some(name) => match self.texture(images, name) {
                Some(t) => Some(t),
                None => return,
            },
            None => None,
        };
        let gl = &self.gl;
        gl.uniform_matrix4fv_with_f32_array(Some(&self.u_model), false, m);
        gl.uniform4fv_with_f32_array(Some(&self.u_color), &d.color);
        gl.uniform1i(Some(&self.u_lit), d.lit as i32);
        gl.uniform1i(Some(&self.u_use_tex), tex.is_some() as i32);
        if let Some(t) = &tex {
            gl.bind_texture(GL::TEXTURE_2D, Some(t));
        }
        let mesh = match mesh {
            MeshKind::Cube => &self.cube,
            MeshKind::Quad => &self.quad,
            MeshKind::Sphere => &self.sphere,
            MeshKind::Cylinder => &self.cylinder,
        };
        gl.bind_vertex_array(Some(&mesh.vao));
        gl.draw_arrays(GL::TRIANGLES, 0, mesh.count);
    }

    /// Keeps the drawing buffer at the canvas's size on screen, for sharp edges.
    fn fit(&mut self) {
        let r = self.canvas.get_bounding_client_rect();
        let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0).min(2.0);
        let w = ((r.width() * dpr) as u32).clamp(320, 4096);
        let h = ((r.height() * dpr) as u32).clamp(240, 3072);
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }
        self.gl.viewport(0, 0, w as i32, h as i32);
        self.aspect = 1024.0 / 768.0;
    }

    pub fn draw(&mut self, images: &Images, v: Option<&GameView>, explosions: &[Explosion], now: f64) {
        self.fit();
        let gl = self.gl.clone();
        gl.clear_color(0.13, 0.12, 0.11, 1.0);
        gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);
        gl.enable(GL::DEPTH_TEST);
        gl.enable(GL::BLEND);
        gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA);
        gl.enable(GL::CULL_FACE);
        gl.use_program(Some(&self.prog));
        gl.active_texture(GL::TEXTURE0);
        let proj = perspective(FOV_Y, self.aspect, 20.0, 4000.0);
        let view = look_at(EYE, TARGET, [0.0, 1.0, 0.0]);
        self.vp = mul(&proj, &view);
        gl.uniform_matrix4fv_with_f32_array(Some(&self.u_vp), false, &self.vp);
        let light = norm([-0.45, 1.0, -0.3]);
        gl.uniform3fv_with_f32_array(Some(&self.u_light), &light);

        let lit = |color: [f32; 3]| Draw { color: [color[0], color[1], color[2], 1.0], tex: None, lit: true };
        let t_s = (now / 1000.0) as f32;

        // Ground: a dark apron, then the original map.
        gl.disable(GL::CULL_FACE);
        self.draw_mesh(images, MeshKind::Quad, &model([512.0, -1.0, 384.0], 0.0, [3200.0, 1.0, 2600.0]), Draw { color: [0.2, 0.18, 0.16, 1.0], tex: None, lit: false });
        self.draw_mesh(images, MeshKind::Quad, &model([512.0, 0.0, 384.0], 0.0, [1024.0, 1.0, 768.0]), Draw { color: [1.0; 4], tex: Some("map"), lit: false });
        // A low rim around the arena.
        for (pos, size) in [
            ([512.0, 6.0, -6.0], [1048.0, 12.0, 12.0]),
            ([512.0, 6.0, 774.0], [1048.0, 12.0, 12.0]),
            ([-6.0, 6.0, 384.0], [12.0, 12.0, 792.0]),
            ([1030.0, 6.0, 384.0], [12.0, 12.0, 792.0]),
        ] {
            self.draw_mesh(images, MeshKind::Cube, &model(pos, 0.0, size), lit([0.32, 0.3, 0.28]));
        }

        // Decals: spawn points and the zone.
        gl.depth_mask(false);
        for e in map::standard() {
            if let MapEntity::SpawnPoint { pos, rotation } = e {
                self.draw_mesh(images, MeshKind::Quad, &model([pos.x, 0.4, pos.y], rotation as f32, [50.0, 1.0, 65.0]), Draw { color: [1.0; 4], tex: Some("tank_spawnpoint"), lit: false });
            }
        }
        if let Some(z) = v.and_then(|v| v.zone) {
            let pulse = 1.0 + 0.04 * (t_s * 3.0).sin();
            self.draw_mesh(images, MeshKind::Quad, &model([z.x, 0.6, z.y], t_s * 20.0, [90.0 * pulse, 1.0, 90.0 * pulse]), Draw { color: [1.0; 4], tex: Some("king_of_the_hill_zone"), lit: false });
        }
        // Soft shadows under everything that stands up.
        if let Some(v) = v {
            for t in &v.tanks {
                self.draw_mesh(images, MeshKind::Quad, &model([t.pos.x + 6.0, 0.8, t.pos.y + 5.0], t.theta, [86.0, 1.0, 64.0]), Draw { color: [1.0, 1.0, 1.0, 0.45], tex: Some("__blob"), lit: false });
            }
            for p in &v.pickups {
                self.draw_mesh(images, MeshKind::Quad, &model([p.pos.x, 0.8, p.pos.y], 0.0, [40.0, 1.0, 40.0]), Draw { color: [1.0, 1.0, 1.0, 0.35], tex: Some("__blob"), lit: false });
            }
        }
        gl.depth_mask(true);
        gl.enable(GL::CULL_FACE);

        // Walls, textured with the original sprites.
        for e in map::standard() {
            if let MapEntity::Wall(r) = e {
                let c = r.center();
                let tex = if r.w == HORIZONTAL_WALL.0 { "horizontal_wall" } else { "vertical_wall" };
                self.draw_mesh(images, MeshKind::Cube, &model([c.x, WALL_HEIGHT / 2.0, c.y], 0.0, [r.w, WALL_HEIGHT, r.h]), Draw { color: [1.0; 4], tex: Some(tex), lit: true });
            }
        }

        let Some(v) = v else {
            gl.bind_vertex_array(None);
            return;
        };

        // Tanks.
        for t in &v.tanks {
            let base = rgb(t.color);
            let dark = [base[0] * 0.55, base[1] * 0.55, base[2] * 0.55];
            let p = [t.pos.x, 0.0, t.pos.y];
            let th = t.theta;
            for side in [-1.0f32, 1.0] {
                self.draw_mesh(images, MeshKind::Cube, &model(offset(p, th, [0.0, 8.0, side * 19.0]), th, [64.0, 16.0, 13.0]), lit([0.16, 0.16, 0.15]));
                // Wheels showing on the outside of each track.
                for k in -2..=2 {
                    let wpos = offset(p, th, [k as f32 * 12.0, 8.0, side * 25.8]);
                    self.draw_mesh(images, MeshKind::Cube, &model(wpos, th, [8.0, 8.0, 1.2]), lit([0.35, 0.35, 0.33]));
                }
            }
            self.draw_mesh(images, MeshKind::Cube, &model(offset(p, th, [0.0, 18.0, 0.0]), th, [54.0, 12.0, 32.0]), lit(base));
            self.draw_mesh(images, MeshKind::Cube, &model(offset(p, th, [24.0, 15.0, 0.0]), th, [8.0, 8.0, 30.0]), lit(dark));

            // The turret turns by its own angle; its barrel points along angle + 90.
            let ta = t.turret_angle + 90.0;
            let tp = [t.turret_pos.x, 0.0, t.turret_pos.y];
            self.draw_mesh(images, MeshKind::Cube, &model(offset(tp, ta, [0.0, 28.0, 0.0]), ta, [26.0, 10.0, 22.0]), lit(dark));
            self.draw_mesh(images, MeshKind::Cube, &model(offset(tp, ta, [-2.0, 34.0, 0.0]), ta, [12.0, 3.0, 12.0]), lit(base));
            self.draw_mesh(images, MeshKind::Cylinder, &model(offset(tp, ta, [8.0, BARREL_Y, 0.0]), ta, [TURRET_LENGTH - 4.0, 6.0, 6.0]), lit([0.2, 0.2, 0.2]));

            // A health bar floating over every tank.
            let hp = (t.health / 100.0).clamp(0.0, 1.0);
            let bar_pos = [t.pos.x, 62.0, t.pos.y - 8.0];
            gl.disable(GL::CULL_FACE);
            self.draw_mesh(images, MeshKind::Cube, &model(bar_pos, 0.0, [44.0, 4.0, 2.0]), Draw { color: [0.05, 0.05, 0.05, 0.8], tex: None, lit: false });
            self.draw_mesh(images, MeshKind::Cube, &model([bar_pos[0] - 22.0 * (1.0 - hp), 62.0, bar_pos[2] + 1.2], 0.0, [44.0 * hp, 3.2, 1.0]), Draw { color: [0.9, 0.12, 0.1, 1.0], tex: None, lit: false });
            gl.enable(GL::CULL_FACE);
        }

        // Pickups: spinning crates wearing the original icons.
        for p in &v.pickups {
            let size = if matches!(p.kind, PickupKind::Weapon(_)) { 30.0 } else { 26.0 };
            let bob = 4.0 * ((t_s * 2.5) + p.id as f32).sin();
            let m = model([p.pos.x, 26.0 + bob, p.pos.y], t_s * 70.0 + p.id as f32 * 13.0, [size, size, size]);
            self.draw_mesh(images, MeshKind::Cube, &m, Draw { color: [1.0; 4], tex: Some(p.kind.sprite()), lit: true });
        }

        // Projectiles glow (unlit).
        for p in &v.projectiles {
            let (size, color): (V3, [f32; 4]) = match p.kind {
                ProjKind::Default => ([11.0, 4.0, 4.0], [1.0, 0.9, 0.35, 1.0]),
                ProjKind::Bounce => ([10.0, 5.0, 5.0], [1.0, 0.45, 0.1, 1.0]),
                ProjKind::Flame => ([9.0, 7.0, 7.0], [1.0, 0.4 + 0.3 * ((p.id % 7) as f32 / 7.0), 0.05, 0.85]),
                ProjKind::Shockwave => ([14.0, 10.0, 10.0], [0.35, 0.6, 1.0, 1.0]),
                ProjKind::ShockwaveSecondary => ([5.0, 5.0, 5.0], [0.5, 0.95, 1.0, 1.0]),
                ProjKind::Shotgun => ([6.0, 3.0, 3.0], [0.85, 0.85, 0.8, 1.0]),
                ProjKind::Slowspeedy => ([16.0, 5.0, 5.0], [0.4, 1.0, 0.9, 1.0]),
            };
            let mesh = if matches!(p.kind, ProjKind::Shockwave | ProjKind::Flame) { MeshKind::Sphere } else { MeshKind::Cube };
            self.draw_mesh(images, mesh, &model([p.pos.x, BARREL_Y, p.pos.y], p.theta, size), Draw { color, tex: None, lit: false });
        }

        // See-through things last: shields and explosions.
        gl.depth_mask(false);
        gl.disable(GL::CULL_FACE);
        for t in &v.tanks {
            if let Some(s) = t.shield {
                let strong = s > 100.0;
                let a = if strong { 0.22 + 0.06 * (t_s * 8.0).sin() } else { 0.28 };
                let color = if strong { [1.0, 1.0, 0.6, a] } else { [0.45, 0.85, 1.0, a] };
                let r = SHIELD_RADIUS * 2.0;
                self.draw_mesh(images, MeshKind::Sphere, &model([t.pos.x, 14.0, t.pos.y], t_s * 30.0, [r, r * 0.8, r]), Draw { color, tex: None, lit: false });
            }
        }
        for e in explosions {
            let k = ((now - e.started) / 1200.0) as f32;
            if !(0.0..1.0).contains(&k) {
                continue;
            }
            let r = 30.0 + 110.0 * k.sqrt();
            let fire = [1.0, 0.75 - 0.5 * k, 0.2 * (1.0 - k), (1.0 - k) * 0.85];
            self.draw_mesh(images, MeshKind::Sphere, &model([e.pos.x, 20.0 + 30.0 * k, e.pos.y], 0.0, [r, r, r]), Draw { color: fire, tex: None, lit: false });
            let smoke_r = r * 1.25;
            self.draw_mesh(images, MeshKind::Sphere, &model([e.pos.x, 40.0 + 50.0 * k, e.pos.y], 0.0, [smoke_r, smoke_r * 0.7, smoke_r]), Draw { color: [0.2, 0.2, 0.2, 0.35 * (1.0 - k)], tex: None, lit: false });
        }
        gl.depth_mask(true);
        gl.bind_vertex_array(None);
    }
}

#[derive(Clone, Copy)]
enum MeshKind {
    Cube,
    Quad,
    Sphere,
    Cylinder,
}
