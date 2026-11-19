//! Glass: the translucent, self-lit shells drawn after the opaque scene
//! (`shaders/shield.wgsl`): the tanks' shield bubbles, the pickups' auras,
//! hot glows. Three shared meshes (a low-poly icosphere, an open column, a
//! flat ring), every shell one instance: at most three draw calls a frame.
//!
//! The shield bubble is a field thrown by the generator welded on the hull:
//! hot amber, a fresnel rim, low-poly panels shimmering, a gentle wobble,
//! and, Gungan style, a ring wave running over the surface from every hit
//! (the last eight per shield, ages passed to the shader).

use std::collections::HashMap;

use arena_core::{Event, PlayerId};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use crate::world::RenderWorld;

/// Hit slots per shield, and shields with ripples per frame.
pub const HITS_PER: usize = 8;
const MAX_RIPPLED: usize = 8;
/// Seconds a ripple runs (matches `RIPPLE_LIFE` in the shader).
const RIPPLE_LIFE: f32 = 1.5;

/// The field's colour: hot amber, like a welding arc seen through smoked glass.
pub const SHIELD_AMBER: Vec3 = Vec3::new(1.0, 0.4, 0.07);

/// The shell meshes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    /// A unit icosphere.
    Sphere = 0,
    /// An open cylinder, radius 1, y 0..1.
    Column = 1,
    /// A flat annulus at y = 0, radius 0.72..1.
    Ring = 2,
}

/// The shader styles (`params.x`).
#[derive(Clone, Copy, Debug)]
pub enum Style {
    Shield = 0,
    Aura = 1,
    Beam = 2,
    GroundRing = 3,
    Glow = 4,
}

/// One shell on the GPU. Matches `GlassIn` in `shield.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlassInstance {
    pub model: [[f32; 4]; 4],
    /// rgb linear emission, a overall opacity.
    pub color: [f32; 4],
    /// x style, y strength, z first hit slot (< 0 none), w seed.
    pub params: [f32; 4],
}

impl GlassInstance {
    pub fn new(model: Mat4, style: Style, color: Vec3, opacity: f32, strength: f32, seed: f32) -> Self {
        Self { model: model.to_cols_array_2d(), color: color.extend(opacity).to_array(), params: [style as u32 as f32, strength, -1.0, seed] }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GlassVertex {
    pos: [f32; 3],
    normal: [f32; 3],
}

struct ShellMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
}

/// The glass pass: its pipeline, meshes, this frame's instances and hits.
pub struct Glass {
    pipeline: wgpu::RenderPipeline,
    meshes: [ShellMesh; 3],
    /// This frame's instances per mesh (kept between frames: no allocation).
    lists: [Vec<GlassInstance>; 3],
    packed: Vec<GlassInstance>,
    batches: [(u32, u32); 3],
    buffer: wgpu::Buffer,
    capacity: u64,
    hits: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

impl Glass {
    pub fn new(device: &wgpu::Device, scene_layout: &wgpu::BindGroupLayout, msaa: u32, format: wgpu::TextureFormat, depth: wgpu::TextureFormat) -> Self {
        let src = format!("{}\n{}", include_str!("shaders/scene.wgsl"), include_str!("shaders/shield.wgsl"));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("scene.wgsl + shield.wgsl"), source: wgpu::ShaderSource::Wgsl(src.into()) });
        let hits_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glass hits layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let hits = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glass hits"),
            size: (HITS_PER * MAX_RIPPLED * 16) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glass hits bind"),
            layout: &hits_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: hits.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glass pipeline layout"),
            bind_group_layouts: &[Some(scene_layout), Some(&hits_layout)],
            immediate_size: 0,
        });
        let premul = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
        };
        let vertex_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let instance_attributes = wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glass pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_glass"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<GlassVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &vertex_attributes }),
                    Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<GlassInstance>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &instance_attributes }),
                ],
            },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState { count: msaa, ..Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_glass"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(premul), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let upload = |label: &str, (v, i): (Vec<GlassVertex>, Vec<u32>)| ShellMesh {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents: bytemuck::cast_slice(&v), usage: wgpu::BufferUsages::VERTEX }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents: bytemuck::cast_slice(&i), usage: wgpu::BufferUsages::INDEX }),
            count: i.len() as u32,
        };
        let meshes = [upload("glass sphere", icosphere(3)), upload("glass column", column(16)), upload("glass ring", ring(32))];
        let capacity = 256;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glass instances"),
            size: capacity * std::mem::size_of::<GlassInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { pipeline, meshes, lists: Default::default(), packed: Vec::with_capacity(256), batches: [(0, 0); 3], buffer, capacity, hits, bind }
    }

    /// Starts a frame: forgets the last frame's shells.
    pub fn begin(&mut self) {
        for l in &mut self.lists {
            l.clear();
        }
    }

    pub fn push(&mut self, shell: Shell, inst: GlassInstance) {
        self.lists[shell as usize].push(inst);
    }

    /// Uploads this frame's shells and hit slots.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, hits: &[[f32; 4]; HITS_PER * MAX_RIPPLED]) {
        self.packed.clear();
        for (k, l) in self.lists.iter().enumerate() {
            self.batches[k] = (self.packed.len() as u32, l.len() as u32);
            self.packed.extend_from_slice(l);
        }
        if self.packed.is_empty() {
            return;
        }
        let need = self.packed.len() as u64;
        if need > self.capacity {
            self.capacity = need.next_power_of_two();
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("glass instances"),
                size: self.capacity * std::mem::size_of::<GlassInstance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.packed));
        queue.write_buffer(&self.hits, 0, bytemuck::cast_slice(hits));
    }

    /// Draws the shells (inside the scene pass, its group 0 bound).
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.packed.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(1, &self.bind, &[]);
        pass.set_vertex_buffer(1, self.buffer.slice(..));
        for (m, &(first, count)) in self.meshes.iter().zip(&self.batches) {
            if count == 0 {
                continue;
            }
            pass.set_vertex_buffer(0, m.vertices.slice(..));
            pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..m.count, 0, first..first + count);
        }
    }

    /// Shells queued this frame (tests).
    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        self.lists.iter().map(|l| l.len()).sum()
    }
}

/// A unit icosphere, `subdiv` times split, smooth normals.
fn icosphere(subdiv: u32) -> (Vec<GlassVertex>, Vec<u32>) {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let mut v: Vec<Vec3> = [
        (-1.0, t, 0.0),
        (1.0, t, 0.0),
        (-1.0, -t, 0.0),
        (1.0, -t, 0.0),
        (0.0, -1.0, t),
        (0.0, 1.0, t),
        (0.0, -1.0, -t),
        (0.0, 1.0, -t),
        (t, 0.0, -1.0),
        (t, 0.0, 1.0),
        (-t, 0.0, -1.0),
        (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| Vec3::new(x, y, z).normalize())
    .collect();
    let mut f: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..subdiv {
        let mut mid: HashMap<(u32, u32), u32> = HashMap::new();
        let mut half = |a: u32, b: u32, v: &mut Vec<Vec3>| -> u32 {
            let key = (a.min(b), a.max(b));
            *mid.entry(key).or_insert_with(|| {
                v.push(((v[a as usize] + v[b as usize]) * 0.5).normalize());
                v.len() as u32 - 1
            })
        };
        let mut next = Vec::with_capacity(f.len() * 4);
        for [a, b, c] in f {
            let ab = half(a, b, &mut v);
            let bc = half(b, c, &mut v);
            let ca = half(c, a, &mut v);
            next.extend_from_slice(&[[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        f = next;
    }
    (v.iter().map(|p| GlassVertex { pos: p.to_array(), normal: p.to_array() }).collect(), f.concat())
}

/// An open cylinder, radius 1, y 0..1.
fn column(segs: u32) -> (Vec<GlassVertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    for s in 0..=segs {
        let a = s as f32 / segs as f32 * std::f32::consts::TAU;
        let n = [a.cos(), 0.0, a.sin()];
        v.push(GlassVertex { pos: [n[0], 0.0, n[2]], normal: n });
        v.push(GlassVertex { pos: [n[0], 1.0, n[2]], normal: n });
    }
    for s in 0..segs {
        let b = s * 2;
        i.extend_from_slice(&[b, b + 1, b + 3, b, b + 3, b + 2]);
    }
    (v, i)
}

/// A flat annulus at y = 0, radius 0.72..1.
fn ring(segs: u32) -> (Vec<GlassVertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    for s in 0..=segs {
        let a = s as f32 / segs as f32 * std::f32::consts::TAU;
        let (c, sn) = (a.cos(), a.sin());
        v.push(GlassVertex { pos: [c * 0.72, 0.0, sn * 0.72], normal: [0.0, 1.0, 0.0] });
        v.push(GlassVertex { pos: [c, 0.0, sn], normal: [0.0, 1.0, 0.0] });
    }
    for s in 0..segs {
        let b = s * 2;
        i.extend_from_slice(&[b, b + 1, b + 3, b, b + 3, b + 2]);
    }
    (v, i)
}

/// One shield's recent hits: unit directions in its bubble frame, and when.
#[derive(Clone, Copy, Default)]
struct HitRing {
    slots: [(Vec3, f32); HITS_PER],
    next: usize,
}

/// A slug stopped by a shield: it slows in the field, glows and is gone.
#[derive(Clone, Copy)]
struct Stopped {
    pos: Vec3,
    vel: Vec3,
    born: f32,
}

/// Client-side shield state: the hits rippling each bubble, the slugs
/// stopped in the fields.
pub struct Shields {
    /// `?shieldfx=1` (a visual test): every tank shielded, hits every
    /// third of a second on the tanks in view.
    pub test: bool,
    test_t: f32,
    rings: HashMap<PlayerId, HitRing>,
    stopped: Vec<Stopped>,
    /// Bubble frames seen last frame (event hits land in them).
    frames: HashMap<PlayerId, Mat4>,
    clock: f32,
    hits: Box<[[f32; 4]; HITS_PER * MAX_RIPPLED]>,
}

/// The bubble's frame round a tank: centred over the hull, turned with
/// it, an ellipsoid a little bigger than the tank and its parts.
pub fn bubble_frame(pos: Vec3, rot: Quat, scale: f32) -> Mat4 {
    let yaw = {
        let f = rot * Vec3::X;
        Quat::from_rotation_y((-f.z).atan2(f.x))
    };
    Mat4::from_scale_rotation_translation(Vec3::new(4.7, 3.1, 3.7) * scale, yaw, pos + Vec3::Y * 0.35 * scale)
}

impl Shields {
    pub fn new() -> Self {
        Self { test: false, test_t: 0.0, rings: HashMap::new(), stopped: Vec::new(), frames: HashMap::new(), clock: 0.0, hits: Box::new([[0.0, 0.0, 0.0, -1.0]; HITS_PER * MAX_RIPPLED]) }
    }

    /// A hit on a shield: a ripple from where it struck, and the slug
    /// caught in the field.
    pub fn event(&mut self, e: &Event) {
        let Event::ShieldHit { player, pos, .. } = e else { return };
        let Some(frame) = self.frames.get(player) else { return };
        let local = frame.inverse().transform_point3(*pos);
        let dir = local.normalize_or(Vec3::X);
        let ring = self.rings.entry(*player).or_default();
        let k = ring.next % HITS_PER;
        ring.slots[k] = (dir, self.clock);
        ring.next = ring.next.wrapping_add(1);
        // Where on the surface it struck, in the world.
        let surface = frame.transform_point3(dir);
        let centre = frame.transform_point3(Vec3::ZERO);
        if self.stopped.len() < 48 {
            self.stopped.push(Stopped { pos: surface, vel: (centre - surface).normalize_or(Vec3::NEG_Y) * 9.0, born: self.clock });
        }
    }

    /// Queues this frame's bubbles (and stopped slugs) into the glass pass.
    /// `focus`: the tank the camera follows (its own field is fainter).
    pub fn draw(&mut self, glass: &mut crate::shield::Glass, world: &RenderWorld, focus: Option<PlayerId>, scale_of: &dyn Fn(PlayerId) -> f32) {
        self.clock = world.time;
        let now = world.time;
        self.frames.clear();
        for t in world.tanks.iter().filter(|t| t.alive) {
            self.frames.insert(t.player, bubble_frame(t.pos, t.rot, scale_of(t.player)));
        }
        self.rings.retain(|p, r| self.frames.contains_key(p) && r.slots.iter().any(|s| now - s.1 < RIPPLE_LIFE && s.1 > 0.0));
        for h in self.hits.iter_mut() {
            h[3] = -1.0;
        }
        let mut rippled = 0;
        if self.test {
            self.test_t += world.dt.clamp(0.0, 0.1);
            if self.test_t > 0.33 {
                self.test_t = 0.0;
                let k = (now * 7.0) as usize;
                let alive: Vec<(PlayerId, Mat4)> = self.frames.iter().map(|(p, f)| (*p, *f)).collect();
                if let Some(&(player, frame)) = alive.get(k % alive.len().max(1)) {
                    let a = now * 3.7;
                    let d = Vec3::new(a.cos(), 0.3 + 0.3 * (a * 1.3).sin(), a.sin()).normalize();
                    self.event(&Event::ShieldHit { player, pos: frame.transform_point3(d), damage: 10.0 });
                }
            }
        }
        let forced: Vec<(PlayerId, f32)> = if self.test { self.frames.keys().map(|p| (*p, 0.8)).collect() } else { Vec::new() };
        let list = if self.test { &forced } else { &world.shields };
        for &(player, strength) in list {
            let Some(frame) = self.frames.get(&player) else { continue };
            let mut inst = GlassInstance::new(*frame, Style::Shield, SHIELD_AMBER * 1.6, 1.0, strength.clamp(0.15, 1.0), player as f32 * 0.37);
            // A weak field stutters.
            if strength < 0.3 {
                let s = (now * 23.0 + player as f32).sin() * (now * 7.3).sin();
                inst.color[3] = 0.6 + 0.4 * s.abs();
            }
            if Some(player) == focus {
                // Your own: there, but not in your eyes.
                inst.color[3] *= 0.6;
            }
            if let Some(r) = self.rings.get(&player).filter(|_| rippled < MAX_RIPPLED) {
                let base = rippled * HITS_PER;
                for (k, s) in r.slots.iter().enumerate() {
                    let age = now - s.1;
                    if s.1 > 0.0 && (0.0..RIPPLE_LIFE).contains(&age) {
                        self.hits[base + k] = s.0.extend(age).to_array();
                    }
                }
                inst.params[2] = base as f32;
                rippled += 1;
            }
            glass.push(Shell::Sphere, inst);
        }
        // Slugs caught in a field: they slow hard, glow white-hot, and fade.
        self.stopped.retain(|s| now - s.born < 0.35 && now >= s.born);
        for s in &self.stopped {
            let age = now - s.born;
            let k = 1.0 - age / 0.35;
            let travel = s.vel * (1.0 - (-age * 9.0).exp()) / 9.0;
            let rot = Quat::from_rotation_arc(Vec3::X, s.vel.normalize_or(Vec3::X));
            let m = Mat4::from_scale_rotation_translation(Vec3::new(0.55, 0.16, 0.16) * (0.5 + 0.5 * k), rot, s.pos + travel);
            glass.push(Shell::Sphere, GlassInstance::new(m, Style::Glow, Vec3::new(9.0, 5.5, 2.0) * k, 1.0, 1.0, 0.0));
        }
    }

    /// This frame's hit slots for the glass pass.
    pub fn hits(&self) -> &[[f32; 4]; HITS_PER * MAX_RIPPLED] {
        &self.hits
    }

    /// Light from fresh hits (the flash on the hull and the sand).
    pub fn lights(&self, out: &mut Vec<crate::renderer::Light>) {
        for s in self.stopped.iter().take(2) {
            let k = 1.0 - (self.clock - s.born) / 0.35;
            out.push(crate::renderer::Light { pos: s.pos, radius: 7.0, color: Vec3::new(1.0, 0.55, 0.2) * 10.0 * k * k });
        }
    }
}

