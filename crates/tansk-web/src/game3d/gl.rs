//! A small WebGL2 toolkit for Tansk 3D: vector math, one shader with a few
//! modes (lit, unlit, terrain, sky, glow, beam), meshes and textures.

use std::collections::HashMap;

use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, HtmlImageElement, WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram, WebGlTexture,
    WebGlUniformLocation, WebGlVertexArrayObject,
};

use crate::assets::Images;

pub type Mat4 = [f32; 16];
pub type V3 = [f32; 3];

// ------------------------------------------------------------------ math

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
/// a + b * k
pub fn madd(a: V3, b: V3, k: f32) -> V3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}
pub fn norm(a: V3) -> V3 {
    let l = len(a).max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// The unit vector for a yaw (0 = +x, 90 = +z) and a pitch (up positive), degrees.
pub fn dir(yaw: f32, pitch: f32) -> V3 {
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    [cy * cp, sp, sy * cp]
}

/// A right-handed frame: forward (local +x), up (local +y), right (local +z).
#[derive(Clone, Copy)]
pub struct Basis {
    pub f: V3,
    pub u: V3,
    pub r: V3,
}

impl Basis {
    pub fn yaw(yaw: f32) -> Basis {
        let (s, c) = yaw.to_radians().sin_cos();
        Basis { f: [c, 0.0, s], u: [0.0, 1.0, 0.0], r: [-s, 0.0, c] }
    }
    /// Nose up by `deg` (about the right axis).
    pub fn pitched(self, deg: f32) -> Basis {
        let (s, c) = deg.to_radians().sin_cos();
        Basis { f: add(scale(self.f, c), scale(self.u, s)), u: sub(scale(self.u, c), scale(self.f, s)), r: self.r }
    }
    /// Right side up by `deg` (about the forward axis).
    pub fn rolled(self, deg: f32) -> Basis {
        let (s, c) = deg.to_radians().sin_cos();
        Basis { f: self.f, u: sub(scale(self.u, c), scale(self.r, s)), r: add(scale(self.r, c), scale(self.u, s)) }
    }
    /// Any frame whose forward is `f`.
    pub fn along(f: V3) -> Basis {
        let f = norm(f);
        let up = if f[1].abs() > 0.95 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let r = norm(cross(f, up));
        Basis { f, u: cross(r, f), r }
    }
    /// A point given in this frame's local coordinates, around `origin`.
    pub fn at(&self, origin: V3, local: V3) -> V3 {
        let p = madd(origin, self.f, local[0]);
        let p = madd(p, self.u, local[1]);
        madd(p, self.r, local[2])
    }
    /// Translate * rotate * scale.
    pub fn model(&self, pos: V3, size: V3) -> Mat4 {
        let (f, u, r) = (self.f, self.u, self.r);
        [
            f[0] * size[0], f[1] * size[0], f[2] * size[0], 0.0,
            u[0] * size[1], u[1] * size[1], u[2] * size[1], 0.0,
            r[0] * size[2], r[1] * size[2], r[2] * size[2], 0.0,
            pos[0], pos[1], pos[2], 1.0,
        ]
    }
}

pub fn identity_at(pos: V3, size: V3) -> Mat4 {
    Basis::yaw(0.0).model(pos, size)
}

pub fn perspective(fovy_deg: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fovy_deg.to_radians() / 2.0).tan();
    [
        f / aspect, 0.0, 0.0, 0.0,
        0.0, f, 0.0, 0.0,
        0.0, 0.0, (far + near) / (near - far), -1.0,
        0.0, 0.0, 2.0 * far * near / (near - far), 0.0,
    ]
}

pub fn look_at(eye: V3, center: V3, up: V3) -> Mat4 {
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

pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut r = [0.0; 16];
    for c in 0..4 {
        for row in 0..4 {
            r[c * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[c * 4 + k]).sum();
        }
    }
    r
}

// --------------------------------------------------------------- shaders

const VS: &str = r#"#version 300 es
layout(location=0) in vec3 a_pos;
layout(location=1) in vec3 a_nrm;
layout(location=2) in vec2 a_uv;
uniform mat4 u_vp;
uniform mat4 u_model;
out vec3 v_nrm;
out vec2 v_uv;
out vec3 v_world;
void main() {
    vec4 w = u_model * vec4(a_pos, 1.0);
    v_world = w.xyz;
    v_nrm = mat3(u_model) * a_nrm;
    v_uv = a_uv;
    gl_Position = u_vp * w;
}"#;

const FS: &str = r#"#version 300 es
precision highp float;
in vec3 v_nrm;
in vec2 v_uv;
in vec3 v_world;
uniform vec4 u_color;
uniform sampler2D u_tex;
uniform int u_use_tex;
uniform int u_mode;
uniform vec3 u_light;
uniform vec3 u_eye;
uniform vec3 u_fog_color;
uniform float u_fog;
uniform vec2 u_uv_scale;
out vec4 o;
void main() {
    if (u_mode == 3) {
        vec3 d = normalize(v_world - u_eye);
        float h = clamp(d.y * 1.4 + 0.04, 0.0, 1.0);
        vec3 sky = mix(u_fog_color, vec3(0.30, 0.52, 0.85), pow(h, 0.6));
        float s = max(dot(d, u_light), 0.0);
        sky += vec3(1.0, 0.92, 0.75) * pow(s, 200.0) * 1.2 + vec3(1.0, 0.85, 0.6) * pow(s, 8.0) * 0.18;
        o = vec4(sky, 1.0);
        return;
    }
    vec4 c = u_color;
    vec3 n = normalize(v_nrm);
    if (u_use_tex == 1) {
        vec2 uv = u_mode == 2 ? v_world.xz * u_uv_scale : v_uv * u_uv_scale;
        c *= texture(u_tex, uv);
        if (c.a < 0.03) discard;
    }
    if (u_mode == 2) {
        float slope = 1.0 - n.y;
        c.rgb = mix(c.rgb, c.rgb * vec3(0.66, 0.58, 0.5), smoothstep(0.08, 0.45, slope));
        c.rgb *= mix(0.92, 1.06, smoothstep(10.0, 80.0, v_world.y));
    }
    if (u_mode == 1 || u_mode == 2) {
        float d = max(dot(n, u_light), 0.0);
        c.rgb *= 0.42 + 0.72 * d;
    }
    if (u_mode == 4) {
        vec3 v = normalize(u_eye - v_world);
        float f = 1.0 - abs(dot(n, v));
        c.a *= 0.2 + 0.8 * f * f;
    }
    if (u_mode == 6) {
        vec3 v = normalize(u_eye - v_world);
        float f = abs(dot(n, v));
        c.a *= f * f;
    }
    if (u_mode == 5) {
        c.a *= (1.0 - v_uv.y) * (1.0 - v_uv.y);
    }
    float dist = length(v_world - u_eye);
    float fog = 1.0 - exp(-pow(dist * u_fog, 2.0));
    c.rgb = mix(c.rgb, u_fog_color, fog);
    o = c;
}"#;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Unlit = 0,
    Lit = 1,
    Terrain = 2,
    Sky = 3,
    Glow = 4,
    Beam = 5,
    /// Soft-edged puffs: opaque in the middle, fading at the rim.
    Soft = 6,
}

pub struct Mesh {
    vao: WebGlVertexArrayObject,
    _vbo: WebGlBuffer,
    _ibo: Option<WebGlBuffer>,
    count: i32,
    indexed: bool,
}

#[derive(Clone, Copy)]
pub enum Blend {
    Alpha,
    Add,
}

/// How to draw one mesh.
#[derive(Clone, Copy)]
pub struct Draw<'a> {
    pub color: [f32; 4],
    pub tex: Option<&'a str>,
    pub mode: Mode,
    /// Texture coordinates are multiplied by this (terrain: world xz).
    pub uv_scale: [f32; 2],
}

impl Draw<'static> {
    pub fn lit(c: [f32; 3]) -> Self {
        Draw { color: [c[0], c[1], c[2], 1.0], tex: None, mode: Mode::Lit, uv_scale: [1.0, 1.0] }
    }
    pub fn unlit(c: [f32; 4]) -> Self {
        Draw { color: c, tex: None, mode: Mode::Unlit, uv_scale: [1.0, 1.0] }
    }
    pub fn glow(c: [f32; 4]) -> Self {
        Draw { color: c, tex: None, mode: Mode::Glow, uv_scale: [1.0, 1.0] }
    }
    pub fn soft(c: [f32; 4]) -> Self {
        Draw { color: c, tex: None, mode: Mode::Soft, uv_scale: [1.0, 1.0] }
    }
}

impl<'a> Draw<'a> {
    pub fn tex(name: &'a str, mode: Mode) -> Self {
        Draw { color: [1.0; 4], tex: Some(name), mode, uv_scale: [1.0, 1.0] }
    }
}

pub struct Gl {
    pub canvas: HtmlCanvasElement,
    pub gl: GL,
    u_vp: WebGlUniformLocation,
    u_model: WebGlUniformLocation,
    u_color: WebGlUniformLocation,
    u_use_tex: WebGlUniformLocation,
    u_mode: WebGlUniformLocation,
    u_light: WebGlUniformLocation,
    u_eye: WebGlUniformLocation,
    u_fog_color: WebGlUniformLocation,
    u_fog: WebGlUniformLocation,
    u_uv_scale: WebGlUniformLocation,
    prog: WebGlProgram,
    textures: HashMap<String, WebGlTexture>,
    pub width: u32,
    pub height: u32,
}

impl Gl {
    pub fn new(canvas: HtmlCanvasElement) -> Option<Gl> {
        let opts = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&opts, &"antialias".into(), &true.into());
        // Opaque: see-through effects must not let the page show through.
        let _ = js_sys::Reflect::set(&opts, &"alpha".into(), &false.into());
        let gl: GL = canvas.get_context_with_context_options("webgl2", &opts).ok()??.dyn_into().ok()?;
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
        Some(Gl {
            u_vp: u("u_vp")?,
            u_model: u("u_model")?,
            u_color: u("u_color")?,
            u_use_tex: u("u_use_tex")?,
            u_mode: u("u_mode")?,
            u_light: u("u_light")?,
            u_eye: u("u_eye")?,
            u_fog_color: u("u_fog_color")?,
            u_fog: u("u_fog")?,
            u_uv_scale: u("u_uv_scale")?,
            prog,
            textures: HashMap::new(),
            width: 0,
            height: 0,
            gl,
            canvas,
        })
    }

    /// Uploads interleaved (pos, normal, uv) vertices, optionally indexed.
    pub fn mesh(&self, data: &[f32], indices: Option<&[u32]>) -> Option<Mesh> {
        let gl = &self.gl;
        let vao = gl.create_vertex_array()?;
        gl.bind_vertex_array(Some(&vao));
        let vbo = gl.create_buffer()?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vbo));
        let arr = js_sys::Float32Array::from(data);
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &arr, GL::STATIC_DRAW);
        let stride = 8 * 4;
        for (loc, size, off) in [(0, 3, 0), (1, 3, 12), (2, 2, 24)] {
            gl.enable_vertex_attrib_array(loc);
            gl.vertex_attrib_pointer_with_i32(loc, size, GL::FLOAT, false, stride, off);
        }
        let (ibo, count, indexed) = match indices {
            Some(idx) => {
                let ibo = gl.create_buffer()?;
                gl.bind_buffer(GL::ELEMENT_ARRAY_BUFFER, Some(&ibo));
                let arr = js_sys::Uint32Array::from(idx);
                gl.buffer_data_with_array_buffer_view(GL::ELEMENT_ARRAY_BUFFER, &arr, GL::STATIC_DRAW);
                (Some(ibo), idx.len() as i32, true)
            }
            None => (None, (data.len() / 8) as i32, false),
        };
        gl.bind_vertex_array(None);
        Some(Mesh { vao, _vbo: vbo, _ibo: ibo, count, indexed })
    }

    /// The sprite `name` as a texture once it has loaded; `repeat` tiles it.
    pub fn texture(&mut self, images: &Images, name: &str, repeat: bool) -> Option<WebGlTexture> {
        let key = if repeat { format!("{name}#r") } else { name.to_string() };
        if let Some(t) = self.textures.get(&key) {
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
        let wrap = if repeat { GL::MIRRORED_REPEAT } else { GL::CLAMP_TO_EDGE } as i32;
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, wrap);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, wrap);
        self.textures.insert(key, t.clone());
        Some(t)
    }

    /// A texture painted on a canvas (e.g. the soft shadow), cached under `name`.
    pub fn canvas_texture(&mut self, name: &str, c: &HtmlCanvasElement) -> Option<WebGlTexture> {
        if let Some(t) = self.textures.get(name) {
            return Some(t.clone());
        }
        let gl = &self.gl;
        let t = gl.create_texture()?;
        gl.bind_texture(GL::TEXTURE_2D, Some(&t));
        gl.tex_image_2d_with_u32_and_u32_and_html_canvas_element(GL::TEXTURE_2D, 0, GL::RGBA as i32, GL::RGBA, GL::UNSIGNED_BYTE, c).ok()?;
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::LINEAR as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);
        self.textures.insert(name.to_string(), t.clone());
        Some(t)
    }

    /// Keeps the drawing buffer at the canvas's on-screen size; starts a frame.
    pub fn begin(&mut self, vp: &Mat4, eye: V3, light: V3, fog_color: V3, fog: f32) {
        let r = self.canvas.get_bounding_client_rect();
        let dpr = web_sys::window().map(|w| w.device_pixel_ratio()).unwrap_or(1.0).min(2.0);
        let w = ((r.width() * dpr) as u32).clamp(320, 4096);
        let h = ((r.height() * dpr) as u32).clamp(240, 3072);
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }
        self.width = w;
        self.height = h;
        let gl = &self.gl;
        gl.viewport(0, 0, w as i32, h as i32);
        gl.clear_color(fog_color[0], fog_color[1], fog_color[2], 1.0);
        gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);
        gl.enable(GL::DEPTH_TEST);
        gl.depth_func(GL::LEQUAL);
        gl.enable(GL::BLEND);
        gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA);
        gl.enable(GL::CULL_FACE);
        gl.depth_mask(true);
        gl.use_program(Some(&self.prog));
        gl.active_texture(GL::TEXTURE0);
        gl.uniform_matrix4fv_with_f32_array(Some(&self.u_vp), false, vp);
        gl.uniform3fv_with_f32_array(Some(&self.u_light), &light);
        gl.uniform3fv_with_f32_array(Some(&self.u_eye), &eye);
        gl.uniform3fv_with_f32_array(Some(&self.u_fog_color), &fog_color);
        gl.uniform1f(Some(&self.u_fog), fog);
    }

    pub fn set_fog(&self, fog: f32) {
        self.gl.uniform1f(Some(&self.u_fog), fog);
    }

    pub fn blend(&self, b: Blend) {
        match b {
            Blend::Alpha => self.gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA),
            Blend::Add => self.gl.blend_func(GL::SRC_ALPHA, GL::ONE),
        }
    }

    /// Depth writes and back-face culling, for see-through things.
    pub fn solid(&self, on: bool) {
        self.gl.depth_mask(on);
        if on {
            self.gl.enable(GL::CULL_FACE);
        } else {
            self.gl.disable(GL::CULL_FACE);
        }
    }

    pub fn draw(&mut self, images: &Images, mesh: &Mesh, m: &Mat4, d: Draw) {
        let tex = match d.tex {
            Some(name) => match self.textures.get(name).cloned() {
                Some(t) => Some(t),
                None => match self.texture(images, name, false) {
                    Some(t) => Some(t),
                    None => return,
                },
            },
            None => None,
        };
        self.draw_with(mesh, m, d, tex.as_ref());
    }

    pub fn draw_with(&self, mesh: &Mesh, m: &Mat4, d: Draw, tex: Option<&WebGlTexture>) {
        let gl = &self.gl;
        gl.uniform_matrix4fv_with_f32_array(Some(&self.u_model), false, m);
        gl.uniform4fv_with_f32_array(Some(&self.u_color), &d.color);
        gl.uniform1i(Some(&self.u_mode), d.mode as i32);
        gl.uniform2f(Some(&self.u_uv_scale), d.uv_scale[0], d.uv_scale[1]);
        gl.uniform1i(Some(&self.u_use_tex), tex.is_some() as i32);
        if let Some(t) = tex {
            gl.bind_texture(GL::TEXTURE_2D, Some(t));
        }
        gl.bind_vertex_array(Some(&mesh.vao));
        if mesh.indexed {
            gl.draw_elements_with_i32(GL::TRIANGLES, mesh.count, GL::UNSIGNED_INT, 0);
        } else {
            gl.draw_arrays(GL::TRIANGLES, 0, mesh.count);
        }
    }

    pub fn end(&self) {
        self.gl.bind_vertex_array(None);
        self.gl.depth_mask(true);
    }
}

// ---------------------------------------------------------------- meshes

pub fn push(v: &mut Vec<f32>, p: V3, n: V3, uv: [f32; 2]) {
    v.extend_from_slice(&[p[0], p[1], p[2], n[0], n[1], n[2], uv[0], uv[1]]);
}

/// A unit cube centred on the origin, each face wearing the whole texture.
pub fn cube_data() -> Vec<f32> {
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
            ([n[0] * 0.5 + u[0] * a + w[0] * b, n[1] * 0.5 + u[1] * a + w[1] * b, n[2] * 0.5 + u[2] * a + w[2] * b], [a + 0.5, b + 0.5])
        };
        let q = [corner(-0.5, -0.5), corner(0.5, -0.5), corner(0.5, 0.5), corner(-0.5, 0.5)];
        for i in [0, 1, 2, 0, 2, 3] {
            push(&mut v, q[i].0, n, q[i].1);
        }
    }
    v
}

/// A unit square facing up (local +y); u along x, v along z.
pub fn quad_data() -> Vec<f32> {
    let mut v = Vec::new();
    let n = [0.0, 1.0, 0.0];
    let q = [([-0.5, 0.0, -0.5], [0.0, 0.0]), ([0.5, 0.0, -0.5], [1.0, 0.0]), ([0.5, 0.0, 0.5], [1.0, 1.0]), ([-0.5, 0.0, 0.5], [0.0, 1.0])];
    for i in [0, 2, 1, 0, 3, 2] {
        push(&mut v, q[i].0, n, q[i].1);
    }
    v
}

/// A unit-diameter sphere.
pub fn sphere_data(rings: usize, segs: usize) -> Vec<f32> {
    let mut v = Vec::new();
    let p = |r: usize, s: usize| -> V3 {
        let th = std::f32::consts::PI * r as f32 / rings as f32;
        let ph = std::f32::consts::TAU * s as f32 / segs as f32;
        [th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin()]
    };
    for r in 0..rings {
        for s in 0..segs {
            let (a, b, c, d) = (p(r, s), p(r + 1, s), p(r + 1, s + 1), p(r, s + 1));
            for q in [a, c, b, a, d, c] {
                push(&mut v, scale(q, 0.5), q, [0.0, 0.0]);
            }
        }
    }
    v
}

/// A unit cylinder along x, from x = 0 to 1, radius 0.5, capped at the far end.
pub fn cylinder_data(segs: usize) -> Vec<f32> {
    let mut v = Vec::new();
    for s in 0..segs {
        let a0 = std::f32::consts::TAU * s as f32 / segs as f32;
        let a1 = std::f32::consts::TAU * (s + 1) as f32 / segs as f32;
        let (y0, z0, y1, z1) = (a0.cos() * 0.5, a0.sin() * 0.5, a1.cos() * 0.5, a1.sin() * 0.5);
        let n0 = [0.0, y0 * 2.0, z0 * 2.0];
        let n1 = [0.0, y1 * 2.0, z1 * 2.0];
        push(&mut v, [0.0, y0, z0], n0, [0.0, 0.0]);
        push(&mut v, [1.0, y1, z1], n1, [0.0, 0.0]);
        push(&mut v, [1.0, y0, z0], n0, [0.0, 0.0]);
        push(&mut v, [0.0, y0, z0], n0, [0.0, 0.0]);
        push(&mut v, [0.0, y1, z1], n1, [0.0, 0.0]);
        push(&mut v, [1.0, y1, z1], n1, [0.0, 0.0]);
        push(&mut v, [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0]);
        push(&mut v, [1.0, y0, z0], [1.0, 0.0, 0.0], [0.0, 0.0]);
        push(&mut v, [1.0, y1, z1], [1.0, 0.0, 0.0], [0.0, 0.0]);
    }
    v
}

/// An open tube along y from 0 to 1, radius 0.5; v runs 0 at the bottom to 1 at the top.
pub fn tube_data(segs: usize) -> Vec<f32> {
    let mut v = Vec::new();
    for s in 0..segs {
        let a0 = std::f32::consts::TAU * s as f32 / segs as f32;
        let a1 = std::f32::consts::TAU * (s + 1) as f32 / segs as f32;
        let (x0, z0, x1, z1) = (a0.cos() * 0.5, a0.sin() * 0.5, a1.cos() * 0.5, a1.sin() * 0.5);
        let n0 = [x0 * 2.0, 0.0, z0 * 2.0];
        let n1 = [x1 * 2.0, 0.0, z1 * 2.0];
        push(&mut v, [x0, 0.0, z0], n0, [0.0, 0.0]);
        push(&mut v, [x1, 1.0, z1], n1, [1.0, 1.0]);
        push(&mut v, [x1, 0.0, z1], n1, [1.0, 0.0]);
        push(&mut v, [x0, 0.0, z0], n0, [0.0, 0.0]);
        push(&mut v, [x0, 1.0, z0], n0, [0.0, 1.0]);
        push(&mut v, [x1, 1.0, z1], n1, [1.0, 1.0]);
    }
    v
}
