//! The wgpu renderer: three-cascade shadow maps, an MSAA HDR forward pass
//! with instanced meshes, effect sprites and rut decals, bloom, tonemap.
//! The same code drives the browser (WebGPU), the native window (Vulkan)
//! and the headless `--shot` path; only the target texture differs.
//!
//! Meshes are uploaded once ([`Renderer::add_mesh`]). The static scene (map,
//! dressing) is handed over once with [`Renderer::set_statics`], its bounding
//! spheres and GPU rows precomputed; every frame the caller hands over the
//! moving [`Instance`]s. Both are culled per view (camera and each cascade:
//! frustum, then tiny-on-screen for the camera and small casters far away for
//! the shadows), sorted by mesh and drawn one instanced call per mesh.

use glam::camera::rh::proj::directx as proj;
use glam::camera::rh::view::look_at_mat4;
use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;

use crate::bloom::Bloom;
use crate::camera::Camera;
use crate::effects::{Rut, Sprite};
use crate::meshkit::MeshBuilder;

pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const CASCADES: usize = 3;

/// Quality presets for the Settings menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // wired to the Settings menu by the game layer
pub enum Quality {
    Low,
    Medium,
    High,
    /// High plus 4x MSAA. MSAA is very costly under WebGPU (Chrome/Dawn on
    /// D3D12 measured about 15 ms a frame at 1080p on an RTX 5070 Ti, against
    /// under 1 ms for the whole frame with FXAA), so only this preset has it.
    Ultra,
}

/// What a preset sets. Medium is the 60 FPS preset for a mid laptop:
/// FXAA instead of MSAA, a render scale with dynamic resolution under it,
/// lighter shadows (the far cascade every other frame), a shorter bloom
/// chain, stricter small-object culling. Low also drops a cascade and runs
/// a lighter fragment shader. High is full resolution and full shadows
/// with FXAA; only Ultra pays for MSAA.
#[derive(Clone, Copy, Debug)]
pub struct QualitySettings {
    pub shadow_size: u32,
    /// Shadow cascades used (2 or 3).
    pub cascades: usize,
    /// The farthest cascade is redrawn every this many frames.
    pub far_shadow_every: u32,
    pub msaa: u32,
    /// Post anti-aliasing (FXAA) instead of MSAA.
    pub fxaa: bool,
    /// The highest render scale; dynamic scaling stays at or below it.
    pub render_scale: f32,
    /// The lowest render scale dynamic scaling may go to.
    pub min_scale: f32,
    /// Bloom mip levels (each half the last, from half resolution).
    pub bloom_levels: usize,
    /// Lighter fragment work: 4-tap shadows, no sphere occlusion, no fine wear.
    pub lite: bool,
    /// Effect sprites (smoke, dust, fire) drawn at half resolution and
    /// composited (needs no MSAA): explosions are overdraw-bound.
    pub half_fx: bool,
    /// On-screen size (pixels of radius) below which the camera skips a mesh.
    pub min_screen_radius: f32,
    /// Shadow casters smaller than `distance / shadow_lod` (m) are skipped.
    pub shadow_lod: f32,
    /// Effect particle cap (see `Fx::set_max_particles`).
    pub max_particles: usize,
    /// Point lights (muzzle flashes, explosions, fires) shaded per pixel.
    pub max_lights: usize,
    /// How far blob contact shadows are drawn from the eye.
    pub blob_range: f32,
}

impl Quality {
    pub fn settings(self) -> QualitySettings {
        match self {
            Quality::Low => QualitySettings {
                shadow_size: 1024,
                cascades: 2,
                far_shadow_every: 2,
                msaa: 1,
                fxaa: true,
                render_scale: 0.75,
                min_scale: 0.5,
                bloom_levels: 3,
                lite: true,
                half_fx: true,
                min_screen_radius: 3.0,
                shadow_lod: 40.0,
                max_particles: 1200,
                max_lights: 3,
                blob_range: 50.0,
            },
            Quality::Medium => QualitySettings {
                shadow_size: 1536,
                cascades: 3,
                far_shadow_every: 2,
                msaa: 1,
                fxaa: true,
                render_scale: 0.85,
                min_scale: 0.6,
                bloom_levels: 4,
                lite: false,
                half_fx: true,
                min_screen_radius: 2.0,
                shadow_lod: 55.0,
                max_particles: 2500,
                max_lights: 5,
                blob_range: 90.0,
            },
            Quality::High => QualitySettings {
                // Not 2048: a 2048 shadow array cost about 4.5 ms in Chrome
                // (Dawn, D3D12) against 0.15 ms at 1792 (measured, RTX 5070 Ti).
                shadow_size: 1792,
                cascades: 3,
                far_shadow_every: 1,
                msaa: 1,
                fxaa: true,
                render_scale: 1.0,
                min_scale: 0.7,
                bloom_levels: 5,
                lite: false,
                half_fx: true,
                min_screen_radius: 1.5,
                shadow_lod: 80.0,
                max_particles: 5000,
                max_lights: 8,
                blob_range: 140.0,
            },
            Quality::Ultra => QualitySettings { msaa: 4, fxaa: false, half_fx: false, ..Quality::High.settings() },
        }
    }
}
const MAX_OCCLUDERS: usize = 32;
const MAX_LIGHTS: usize = 8;
/// Extra depth behind each cascade towards the sun, for tall casters.
const SHADOW_BACK: f32 = 160.0;

/// Time-of-day presets for [`Look`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // wired to the Settings menu by the game layer
pub enum TimeOfDay {
    /// The default: a low golden sun, long warm shadows.
    GoldenHour,
    Dusk,
    Noon,
}

/// Every number that sets the look, in one place. The sun is set by
/// `sun_elevation` / `sun_azimuth` (degrees; azimuth 0 = +X, 90 = +Z).
#[derive(Clone, Debug)]
pub struct Look {
    pub sun_elevation: f32,
    pub sun_azimuth: f32,
    /// Derived from the angles every frame.
    pub sun_dir: Vec3,
    pub sun_color: Vec3,
    /// The blue sky's brightness on screen (divided by the exposure, so a
    /// darker exposure for a brighter sun keeps the sky blue, not navy).
    pub sky_gain: f32,
    /// Ambient light from the sky (the hemisphere's upper half).
    pub sky_color: Vec3,
    pub ground_color: Vec3,
    pub haze: Vec3,
    pub haze_start: f32,
    pub haze_density: f32,
    pub height_fog: f32,
    pub height_falloff: f32,
    pub mie_g: f32,
    /// How strongly the sun lights the dust in the air.
    pub air_glow: f32,
    pub shadow_softness: f32,
    /// Cascade far distances.
    pub splits: [f32; 3],
    pub rim: f32,
    pub spec: f32,
    pub exposure: f32,
    pub vignette: f32,
    pub saturation: f32,
    pub warmth: Vec3,
    /// Bloom added on top (not mixed in), and the threshold it starts at,
    /// both after exposure: only the sun, emissives and fire should reach it.
    pub bloom: f32,
    pub bloom_threshold: f32,
    /// How much each wider bloom level adds (lower: a tighter glow).
    pub bloom_scatter: f32,
    /// Contrast round mid grey, before the curve (1 = none).
    pub contrast: f32,
    /// 0 ACES, 1 AgX.
    pub curve: u32,
    /// Sandstorm intensity 0..1 (see `Renderer::set_storm`).
    pub storm: f32,
}

impl Look {
    pub fn sun_direction(&self) -> Vec3 {
        let (e, a) = (self.sun_elevation.to_radians(), self.sun_azimuth.to_radians());
        Vec3::new(a.cos() * e.cos(), e.sin(), a.sin() * e.cos())
    }

    #[allow(dead_code)] // wired to the Settings menu by the game layer
    pub fn preset(t: TimeOfDay) -> Look {
        let mut l = Look::default();
        match t {
            TimeOfDay::GoldenHour => {}
            TimeOfDay::Dusk => {
                l.sun_elevation = 8.0;
                l.sun_color = Vec3::new(1.0, 0.55, 0.28) * 2.2;
                l.sky_color = Vec3::new(0.36, 0.38, 0.62) * 0.5;
                l.ground_color = Vec3::new(0.9, 0.45, 0.25) * 0.56;
                l.haze = Vec3::new(1.0, 0.66, 0.46) * 1.1;
                l.warmth = Vec3::new(1.04, 0.99, 0.93);
                l.exposure = 0.5;
                l.sky_gain = 0.4;
            }
            TimeOfDay::Noon => {
                l.sun_elevation = 72.0;
                // A high sun in a near-neutral white (short shadows), with a
                // grade that still keeps the sand's (and the salt flat's) colour.
                l.sun_color = Vec3::new(1.0, 0.93, 0.82) * 2.6;
                l.sky_color = Vec3::new(0.42, 0.5, 0.7) * 0.55;
                l.ground_color = Vec3::new(0.85, 0.68, 0.5) * 0.6;
                l.haze = Vec3::new(0.96, 0.89, 0.78) * 1.2;
                l.warmth = Vec3::new(1.02, 1.0, 0.95);
                l.saturation = 1.1;
                l.exposure = 0.205;
            }
        }
        l.sun_dir = l.sun_direction();
        l
    }
}

impl Default for Look {
    fn default() -> Self {
        // Golden hour (the default, and most matches): a low warm sun 13
        // degrees up, long shadows. Noon and dusk differ clearly from it.
        let (elev, az) = (13.0f32, 140.0f32);
        let sun_dir = Vec3::new(az.to_radians().cos() * elev.to_radians().cos(), elev.to_radians().sin(), az.to_radians().sin() * elev.to_radians().cos());
        Self {
            sun_elevation: elev,
            sun_azimuth: az,
            sun_dir,
            sun_color: Vec3::new(1.0, 0.74, 0.48) * 2.9,
            sky_gain: 0.5,
            // Generous fill, so faces turned from the sun (dune and canyon
            // walls) still read in shadow.
            sky_color: Vec3::new(0.40, 0.48, 0.70) * 0.55,
            ground_color: Vec3::new(0.92, 0.62, 0.38) * 0.68,
            haze: Vec3::new(1.0, 0.84, 0.64) * 1.25,
            haze_start: 140.0,
            haze_density: 0.0018,
            height_fog: 0.00035,
            height_falloff: 0.18,
            mie_g: 0.85,
            air_glow: 0.08,
            shadow_softness: 1.3,
            splits: [24.0, 75.0, 230.0],
            rim: 1.0,
            spec: 1.0,
            exposure: 0.32,
            vignette: 0.3,
            saturation: 0.97,
            warmth: Vec3::new(1.05, 1.0, 0.91),
            bloom: 0.05,
            bloom_threshold: 1.0,
            bloom_scatter: 0.55,
            contrast: 1.7,
            storm: 0.0,
            curve: 0,
        }
    }
}

/// A mesh uploaded with [`Renderer::add_mesh`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshId(pub u32);

/// One placed mesh. `params` reaches the shader per instance (x/z tread
/// scroll, y hit flash, w damage). `tint` (linear rgb, a = 1 to use it)
/// repaints the mesh's team paint, so every crew's tanks share one mesh per
/// part instead of one per colour (fewer draw calls).
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pub mesh: MeshId,
    pub model: Mat4,
    pub params: Vec4,
    pub tint: Vec4,
}

/// Flags an instance's tint so the see-through never dithers it (the
/// followed tank): `tint.w` 1 (tinted) becomes 2, 0 (untinted) becomes -1;
/// the shader still reads `w > 0.5` as "use the tint".
pub fn no_see_through(tint: Vec4) -> Vec4 {
    let w = if tint.w > 0.5 { 2.0 } else { -1.0 };
    Vec4::new(tint.x, tint.y, tint.z, w)
}

impl Instance {
    pub fn new(mesh: MeshId, model: Mat4) -> Self {
        Self { mesh, model, params: Vec4::ZERO, tint: Vec4::ZERO }
    }

    /// With per-instance params and a team-paint tint.
    pub fn tinted(mesh: MeshId, model: Mat4, params: Vec4, tint: Vec4) -> Self {
        Self { mesh, model, params, tint }
    }
}

/// A point light (flashes, fires).
#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub pos: Vec3,
    pub radius: f32,
    pub color: Vec3,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuInstance {
    model: [[f32; 4]; 4],
    params: [f32; 4],
    tint: [f32; 4],
}

/// An instance ready to cull and pack: its world bounding sphere and GPU row.
#[derive(Clone, Copy)]
struct Placed {
    mesh: MeshId,
    centre: Vec3,
    radius: f32,
    gpu: GpuInstance,
}

/// Meshes bigger than this (vertices) are not kept by the CPU recorder.
pub const RECORD_MAX_VERTS: usize = 12_000;

/// Side of the cells the static scene is culled by first, m.
const STATIC_CELL: f32 = 48.0;


#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    cascades: [[[f32; 4]; 4]; CASCADES],
    camera: [f32; 4],
    cam_right: [f32; 4],
    cam_up: [f32; 4],
    sun_dir: [f32; 4],
    sun_color: [f32; 4],
    sky_color: [f32; 4],
    ground_color: [f32; 4],
    fog: [f32; 4],
    fog2: [f32; 4],
    shadow: [f32; 4],
    splits: [f32; 4],
    texels: [f32; 4],
    ranges: [f32; 4],
    misc: [f32; 4],
    occluders: [[f32; 4]; MAX_OCCLUDERS],
    light_pos: [[f32; 4]; MAX_LIGHTS],
    light_col: [[f32; 4]; MAX_LIGHTS],
    /// xyz the see-through focus (the followed tank), w its strength 0..1.
    focus: [f32; 4],
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    centre: Vec3,
    radius: f32,
}

struct Targets {
    /// `None` without MSAA: the pass draws straight into `hdr`.
    msaa_color: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
    #[allow(dead_code)]
    hdr: wgpu::TextureView,
    post_bind: wgpu::BindGroup,
    /// With FXAA: the tonemapped frame at the render scale, and the FXAA
    /// pass's bind group reading it.
    #[allow(dead_code)]
    ldr: Option<wgpu::TextureView>,
    fxaa_bind: Option<wgpu::BindGroup>,
    fx: Option<FxTargets>,
}

/// The half-resolution effects pass: the sprite pipeline drawing into it
/// (testing the scene depth itself) and the composite onto the frame.
struct HalfFx {
    pipeline: wgpu::RenderPipeline,
    depth_layout: wgpu::BindGroupLayout,
    mix_pipeline: wgpu::RenderPipeline,
    mix_layout: wgpu::BindGroupLayout,
}

/// The half-resolution effects target and its bind groups.
struct FxTargets {
    color: wgpu::TextureView,
    depth_bind: wgpu::BindGroup,
    mix_bind: wgpu::BindGroup,
}

/// The FXAA pass's pipeline, layout and uniform.
struct FxaaPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
}

/// The tonemapped intermediate FXAA reads (sRGB-encoded by the shader).
const LDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A grown-on-demand GPU buffer.
struct Dynamic {
    buffer: wgpu::Buffer,
    capacity: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl Dynamic {
    fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages, capacity: u64) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_DST;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: capacity, usage, mapped_at_creation: false });
        Self { buffer, capacity, usage, label }
    }

    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) {
        if bytes.len() as u64 > self.capacity {
            self.capacity = (bytes.len() as u64).next_power_of_two();
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some(self.label), size: self.capacity, usage: self.usage, mapped_at_creation: false });
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytes);
        }
    }
}

/// Timestamps per frame: shadows begin, scene begin, scene end, post
/// begin, post end (the last pass, FXAA when on).
const TIMESTAMPS: u32 = 5;
/// Readback buffers in flight.
const TIMER_SLOTS: usize = 4;

/// GPU pass timing with timestamp queries, where the device has them
/// (WebGPU's `timestamp-query`, Vulkan, D3D12). Averages are logged every
/// couple of seconds and feed the dynamic render scale.
struct GpuTimer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    /// Readback buffers and their state: 0 free, 1 waiting for the map, 2 mapped, 3 failed.
    slots: Vec<(wgpu::Buffer, std::sync::Arc<std::sync::atomic::AtomicU8>)>,
    /// The slot this frame writes, if one was free.
    current: Option<usize>,
    /// Nanoseconds per tick.
    period: f32,
    sums: [f64; 4],
    count: u32,
}

impl GpuTimer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let set = device.create_query_set(&wgpu::QuerySetDescriptor { label: Some("gpu timer"), ty: wgpu::QueryType::Timestamp, count: TIMESTAMPS });
        let size = TIMESTAMPS as u64 * 8;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor { label: Some("gpu timer resolve"), size, usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
        let slots = (0..TIMER_SLOTS)
            .map(|_| {
                let b = device.create_buffer(&wgpu::BufferDescriptor { label: Some("gpu timer readback"), size, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
                (b, std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0)))
            })
            .collect();
        Some(Self { set, resolve, slots, current: None, period: queue.get_timestamp_period(), sums: [0.0; 4], count: 0 })
    }

    /// Reads finished slots; returns the averages (ms: total, shadows,
    /// scene, bloom, post) once enough frames are in.
    fn collect(&mut self) -> Option<[f32; 5]> {
        use std::sync::atomic::Ordering;
        let mut done = None;
        for (buffer, state) in &self.slots {
            match state.load(Ordering::Acquire) {
                2 => {
                    let t: Vec<u64> = {
                        let view = buffer.slice(..).get_mapped_range().ok()?;
                        bytemuck::cast_slice::<u8, u64>(&view).to_vec()
                    };
                    buffer.unmap();
                    state.store(0, Ordering::Release);
                    let ms = |a: usize, b: usize| t[b].saturating_sub(t[a]) as f64 * self.period as f64 * 1e-6;
                    let parts = [ms(0, 1), ms(1, 2), ms(2, 3), ms(3, 4)];
                    // Skip nonsense (a reset or a wrapped counter).
                    if t[4] >= t[0] && parts.iter().sum::<f64>() < 1000.0 {
                        for (s, p) in self.sums.iter_mut().zip(parts) {
                            *s += p;
                        }
                        self.count += 1;
                    }
                }
                3 => state.store(0, Ordering::Release),
                _ => {}
            }
        }
        if self.count >= 60 {
            let n = self.count as f64;
            let a = self.sums.map(|s| (s / n) as f32);
            done = Some([a.iter().sum(), a[0], a[1], a[2], a[3]]);
            self.sums = [0.0; 4];
            self.count = 0;
        }
        done
    }

    /// Picks a free slot for this frame (none: no timestamps this frame).
    fn begin(&mut self) {
        use std::sync::atomic::Ordering;
        self.current = self.slots.iter().position(|(_, s)| s.load(Ordering::Acquire) == 0);
    }

    fn writes(&self, begin: Option<u32>, end: Option<u32>) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.current.map(|_| wgpu::RenderPassTimestampWrites { query_set: &self.set, beginning_of_pass_write_index: begin, end_of_pass_write_index: end })
    }

    fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(i) = self.current {
            encoder.resolve_query_set(&self.set, 0..TIMESTAMPS, &self.resolve, 0);
            encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.slots[i].0, 0, TIMESTAMPS as u64 * 8);
        }
    }

    /// After submit: map the slot written this frame.
    fn after_submit(&mut self) {
        use std::sync::atomic::Ordering;
        if let Some(i) = self.current.take() {
            let (buffer, state) = &self.slots[i];
            state.store(1, Ordering::Release);
            let state = state.clone();
            buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| state.store(if r.is_ok() { 2 } else { 3 }, Ordering::Release));
        }
    }
}

#[allow(dead_code)] // some fields are read only by the game-facing quality API
pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub look: Look,
    width: u32,
    height: u32,
    globals: wgpu::Buffer,
    post_uniform: wgpu::Buffer,
    scene_bind: wgpu::BindGroup,
    cascade_binds: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    shadow_layers: Vec<wgpu::TextureView>,
    shadow_pipeline: wgpu::RenderPipeline,
    scene_pipeline: wgpu::RenderPipeline,
    background_pipeline: wgpu::RenderPipeline,
    sprite_pipeline: wgpu::RenderPipeline,
    /// Translucent shells: shield bubbles, pickup auras, glows (see `shield`).
    pub glass: crate::shield::Glass,
    rut_pipeline: wgpu::RenderPipeline,
    post_pipeline: wgpu::RenderPipeline,
    post_layout: wgpu::BindGroupLayout,
    encode_srgb: bool,
    /// Uploaded meshes; `None` slots were freed and are reused.
    meshes: Vec<Option<GpuMesh>>,
    free_slots: Vec<u32>,
    /// While a scope is open, `add_mesh` records its ids here.
    scope: Option<Vec<MeshId>>,
    instances: Dynamic,
    sprites: Dynamic,
    ruts: Dynamic,
    targets: Targets,
    bloom: Bloom,
    blob_pipeline: wgpu::RenderPipeline,
    blobs: Dynamic,
    /// Draw statistics of the last frame: (draw calls, instances).
    pub stats: (u32, u32),
    /// Sprites drawn last frame.
    pub sprite_count: u32,
    /// The static scene, grouped by map cell (see `set_statics`).
    statics: Vec<Placed>,
    /// Bounding sphere and `statics` range of each cell.
    static_cells: Vec<(Vec3, f32, u32, u32)>,
    /// Scratch for this frame's moving instances (kept to reuse its memory).
    moving: Vec<Placed>,
    /// While set, `add_mesh` keeps a CPU copy of each mesh here (the map
    /// build uses them to merge dressing into chunks).
    pub recorded: Option<std::collections::HashMap<MeshId, MeshBuilder>>,
    /// CPU ms the last frame spent culling and packing instances.
    pub cull_ms: f64,
    /// CPU ms the last frame spent on buffer uploads, encoding passes, and submit.
    pub phase_ms: [f64; 3],
    quality: Quality,
    settings: QualitySettings,
    output_format: wgpu::TextureFormat,
    /// Current render scale (dynamic, <= the preset's).
    scale: f32,
    /// Smoothed frame time, and seconds until scale may change again.
    frame_avg: f32,
    scale_cooldown: f32,
    /// Frame time to hold, seconds (default 1/60).
    pub frame_budget: f32,
    fxaa: Option<FxaaPass>,
    half_fx: Option<HalfFx>,
    timer: Option<GpuTimer>,
    /// The last GPU time averages, ms: total, shadows, scene, bloom, post
    /// (zero without timestamp queries).
    pub gpu_ms: [f32; 5],
    /// A new `gpu_ms` average arrived since the last scale decision.
    gpu_fresh: bool,
    gpu_logged: f64,
    frames: u64,
    /// The cascades as last drawn (the far one is not redrawn every frame).
    cascade_state: [(Mat4, Mat4, f32); CASCADES],
    /// See-through: what the camera follows, and how strongly whatever
    /// stands between it and the eye is dithered away (eased 0..1).
    focus: Vec3,
    focus_strength: f32,
    /// See-through on (shots turn it off for a before/after).
    pub see_through_on: bool,
    /// The map is a pale salt flat (the Refinery): a high sun gets a
    /// darker, warmer, more saturated grade so it does not wash to white.
    pub salt_flat: bool,
    /// The map is a deep gorge (Canyon): walls and floor sit in the
    /// shade much of the day, so the sky and bounce fill are lifted.
    pub gorge: bool,
}

const INSTANCE_STRIDE: u64 = std::mem::size_of::<GpuInstance>() as u64;

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

impl Renderer {
    /// `output_format` is what the final pass writes (the surface's, or the
    /// offscreen PNG target's). If it is not an sRGB format, the tonemap
    /// encodes sRGB itself, which is the usual case on WebGPU canvases.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, output_format: wgpu::TextureFormat, width: u32, height: u32) -> Self {
        Self::with_quality(device, queue, output_format, width, height, Quality::High)
    }

    pub fn with_quality(device: wgpu::Device, queue: wgpu::Queue, output_format: wgpu::TextureFormat, width: u32, height: u32, quality: Quality) -> Self {
        Self::with_settings(device, queue, output_format, width, height, quality, quality.settings())
    }

    /// Like `with_quality` with the preset's settings overridden (benchmarks).
    pub fn with_settings(device: wgpu::Device, queue: wgpu::Queue, output_format: wgpu::TextureFormat, width: u32, height: u32, quality: Quality, mut settings: QualitySettings) -> Self {
        // `?scale=full` (web, benchmarks): render at full size, no dynamic
        // resolution, so a big viewport stands in for a weaker GPU.
        if url_has("scale=full") {
            settings.render_scale = 1.0;
            settings.min_scale = 1.0;
        }
        let shadow_size = settings.shadow_size;
        let msaa_count = settings.msaa;
        let scene_src = include_str!("shaders/scene.wgsl");
        let scene_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene.wgsl"),
            source: wgpu::ShaderSource::Wgsl(scene_src.into()),
        });
        let fx_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene.wgsl + fx.wgsl"),
            source: wgpu::ShaderSource::Wgsl(format!("{scene_src}\n{}", include_str!("shaders/fx.wgsl")).into()),
        });
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/post.wgsl").into()),
        });

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow cascades"),
            size: wgpu::Extent3d { width: shadow_size, height: shadow_size, depth_or_array_layers: CASCADES as u32 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_array = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("shadow array view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let shadow_layers = (0..CASCADES as u32)
            .map(|i| {
                shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("shadow layer"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: i,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
                // Vertex too: effect sprites take their shadow per vertex.
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let cascade_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cascade layout"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX)],
        });
        let scene_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene bind"),
            layout: &scene_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_array) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_sampler) },
            ],
        });
        let cascade_binds = (0..CASCADES)
            .map(|_| {
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("cascade matrix"),
                    size: 64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("cascade bind"),
                    layout: &cascade_layout,
                    entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
                });
                (buffer, bind)
            })
            .collect();

        let vertex_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x4];
        let instance_attributes = wgpu::vertex_attr_array![4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4];
        let mesh_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<crate::meshkit::Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &vertex_attributes,
            }),
            Some(wgpu::VertexBufferLayout { array_stride: INSTANCE_STRIDE, step_mode: wgpu::VertexStepMode::Instance, attributes: &instance_attributes }),
        ];

        let scene_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene pipeline layout"),
            bind_group_layouts: &[Some(&scene_layout)],
            immediate_size: 0,
        });
        let shadow_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow pipeline layout"),
            bind_group_layouts: &[None, Some(&cascade_layout)],
            immediate_size: 0,
        });

        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow pipeline"),
            layout: Some(&shadow_pl),
            vertex: wgpu::VertexState { module: &scene_shader, entry_point: Some("vs_shadow"), compilation_options: Default::default(), buffers: &mesh_buffers },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: SHADOW_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 1.5, clamp: 0.0 },
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });

        let msaa = wgpu::MultisampleState { count: msaa_count, ..Default::default() };
        let hdr_target = |blend: Option<wgpu::BlendState>| [Some(wgpu::ColorTargetState { format: HDR_FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })];
        let opaque_targets = hdr_target(None);
        let depth_state = |write: bool, compare: wgpu::CompareFunction| {
            Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(write),
                depth_compare: Some(compare),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            })
        };
        let scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene pipeline"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState { module: &scene_shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &mesh_buffers },
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: depth_state(true, wgpu::CompareFunction::Less),
            multisample: msaa,
            fragment: Some(wgpu::FragmentState { module: &scene_shader, entry_point: Some("fs_main"), compilation_options: Default::default(), targets: &opaque_targets }),
            multiview_mask: None,
            cache: None,
        });
        let background_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("background pipeline"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState { module: &scene_shader, entry_point: Some("vs_background"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: depth_state(false, wgpu::CompareFunction::LessEqual),
            multisample: msaa,
            fragment: Some(wgpu::FragmentState { module: &scene_shader, entry_point: Some("fs_background"), compilation_options: Default::default(), targets: &opaque_targets }),
            multiview_mask: None,
            cache: None,
        });
        // Premultiplied alpha: puffs blend, glows (alpha 0) add.
        let premul = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
        };
        let blended_targets = hdr_target(Some(premul));
        let sprite_attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
        let sprite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite pipeline"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState {
                module: &fx_shader,
                entry_point: Some("vs_sprite"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Sprite>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &sprite_attributes,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: depth_state(false, wgpu::CompareFunction::Less),
            multisample: msaa,
            fragment: Some(wgpu::FragmentState { module: &fx_shader, entry_point: Some("fs_sprite"), compilation_options: Default::default(), targets: &blended_targets }),
            multiview_mask: None,
            cache: None,
        });
        let glass = crate::shield::Glass::new(&device, &scene_layout, msaa_count, HDR_FORMAT, DEPTH_FORMAT);
        let half_fx = (settings.half_fx && msaa_count == 1).then(|| {
            let depth_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("fx depth layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                }],
            });
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("half fx pipeline layout"),
                bind_group_layouts: &[Some(&scene_layout), Some(&depth_layout)],
                immediate_size: 0,
            });
            // Colour accumulates premultiplied; alpha keeps what still
            // shows through (starts at 1; glows, alpha 0, leave it).
            let accumulate = wgpu::BlendState {
                color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
            };
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("half fx pipeline"),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &fx_shader,
                    entry_point: Some("vs_sprite"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Sprite>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &sprite_attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState { module: &fx_shader, entry_point: Some("fs_sprite_half"), compilation_options: Default::default(), targets: &hdr_target(Some(accumulate)) }),
                multiview_mask: None,
                cache: None,
            });
            let mix_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("fxmix.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/fxmix.wgsl").into()),
            });
            let mix_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("fx mix layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                ],
            });
            let mix_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("fx mix pipeline layout"), bind_group_layouts: &[Some(&mix_layout)], immediate_size: 0 });
            // frame = frame * fx.alpha + fx.rgb
            let over = wgpu::BlendState {
                color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::SrcAlpha, operation: wgpu::BlendOperation::Add },
                alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
            };
            let mix_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("fx mix pipeline"),
                layout: Some(&mix_pl),
                vertex: wgpu::VertexState { module: &mix_shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState { module: &mix_shader, entry_point: Some("fs_main"), compilation_options: Default::default(), targets: &hdr_target(Some(over)) }),
                multiview_mask: None,
                cache: None,
            });
            HalfFx { pipeline, depth_layout, mix_pipeline, mix_layout }
        });
        let rut_attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4];
        let rut_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rut pipeline"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState {
                module: &fx_shader,
                entry_point: Some("vs_rut"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Rut>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &rut_attributes,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: depth_state(false, wgpu::CompareFunction::LessEqual),
            multisample: msaa,
            fragment: Some(wgpu::FragmentState { module: &fx_shader, entry_point: Some("fs_rut"), compilation_options: Default::default(), targets: &blended_targets }),
            multiview_mask: None,
            cache: None,
        });

        let post_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("post uniform"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let post_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post pipeline layout"),
            bind_group_layouts: &[Some(&post_layout)],
            immediate_size: 0,
        });
        // With FXAA the tonemap writes the LDR intermediate; else the output.
        let post_format = if settings.fxaa { LDR_FORMAT } else { output_format };
        let post_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("post pipeline"),
            layout: Some(&post_pl),
            vertex: wgpu::VertexState { module: &post_shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &post_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: post_format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let fxaa = settings.fxaa.then(|| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("fxaa.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/fxaa.wgsl").into()),
            });
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("fxaa layout"),
                entries: &[
                    uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("fxaa pipeline layout"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("fxaa pipeline"),
                layout: Some(&pl),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: output_format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            });
            let uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("fxaa uniform"),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            FxaaPass { pipeline, layout, uniform }
        });

        let instances = Dynamic::new(&device, "instances", wgpu::BufferUsages::VERTEX, INSTANCE_STRIDE * 4096);
        let sprites = Dynamic::new(&device, "sprites", wgpu::BufferUsages::VERTEX, 64 * 1024);
        let ruts = Dynamic::new(&device, "ruts", wgpu::BufferUsages::VERTEX, 32 * 1024);
        let blob_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blob pipeline"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState {
                module: &fx_shader,
                entry_point: Some("vs_blob"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 16,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: depth_state(false, wgpu::CompareFunction::LessEqual),
            multisample: msaa,
            fragment: Some(wgpu::FragmentState { module: &fx_shader, entry_point: Some("fs_blob"), compilation_options: Default::default(), targets: &blended_targets }),
            multiview_mask: None,
            cache: None,
        });
        let blobs = Dynamic::new(&device, "blobs", wgpu::BufferUsages::VERTEX, 16 * 1024);
        let mut bloom = Bloom::new(&device, settings.bloom_levels);
        let scale = settings.render_scale;
        let (iw, ih) = scaled(width, height, scale);
        let targets = make_targets(&device, &post_layout, &post_uniform, &mut bloom, iw, ih, msaa_count, fxaa.as_ref(), half_fx.as_ref());
        let timer = GpuTimer::new(&device, &queue);
        Self {
            device,
            queue,
            look: Look::default(),
            width,
            height,
            globals,
            post_uniform,
            scene_bind,
            cascade_binds,
            shadow_layers,
            shadow_pipeline,
            scene_pipeline,
            background_pipeline,
            sprite_pipeline,
            glass,
            rut_pipeline,
            post_pipeline,
            post_layout,
            encode_srgb: !output_format.is_srgb(),
            meshes: Vec::new(),
            free_slots: Vec::new(),
            scope: None,
            instances,
            sprites,
            ruts,
            targets,
            bloom,
            blob_pipeline,
            blobs,
            stats: (0, 0),
            sprite_count: 0,
            statics: Vec::new(),
            static_cells: Vec::new(),
            moving: Vec::new(),
            recorded: None,
            cull_ms: 0.0,
            phase_ms: [0.0; 3],
            quality,
            settings,
            output_format,
            scale,
            frame_avg: 1.0 / 60.0,
            scale_cooldown: 2.0,
            frame_budget: 1.0 / 60.0,
            fxaa,
            half_fx,
            timer,
            gpu_ms: [0.0; 5],
            gpu_fresh: false,
            gpu_logged: -1e9,
            frames: 0,
            cascade_state: [(Mat4::IDENTITY, Mat4::IDENTITY, 1.0); CASCADES],
            focus: Vec3::ZERO,
            focus_strength: 0.0,
            see_through_on: true,
            salt_flat: false,
            gorge: false,
        }
    }

    /// See-through for the chase camera: picks the tank the camera follows
    /// (the nearest candidate 2..22 m ahead within 4 m of the view axis) and
    /// eases the dither of whatever stands between the eye and it (props,
    /// walls, containers; not the ground) in, or back out when there is no
    /// such tank (gunsight, spectating from afar). Call once a frame.
    /// Returns the followed tank, whose instances the caller flags with
    /// [`no_see_through`].
    pub fn see_through(&mut self, camera: &Camera, tanks: impl Iterator<Item = (u8, Vec3)>, dt: f32) -> Option<u8> {
        let fwd = (camera.target - camera.eye).normalize_or_zero();
        let mut best: Option<(f32, Vec3, u8)> = None;
        for (id, p) in tanks {
            let c = p + Vec3::Y * 1.0;
            let along = (c - camera.eye).dot(fwd);
            if !(2.0..22.0).contains(&along) {
                continue;
            }
            let off = (c - camera.eye - fwd * along).length();
            // The nearest tank ahead (the one the chase rig sits behind),
            // not the one nearest the view axis: an enemy dead ahead
            // further out would otherwise steal the focus.
            if off < 4.0 && best.is_none_or(|(b, _, _)| along < b) {
                best = Some((along, c, id));
            }
        }
        let want = if !self.see_through_on {
            0.0
        } else if let Some((_, c, _)) = best {
            self.focus = c;
            1.0
        } else {
            0.0
        };
        // A still frame (dt 0: shots) takes the target at once.
        let k = if dt > 0.0 { 1.0 - (-dt * 8.0).exp() } else { 1.0 };
        self.focus_strength += (want - self.focus_strength) * k;
        best.map(|b| b.2)
    }

    #[allow(dead_code)] // wired to the Settings menu by the game layer
    pub fn quality(&self) -> Quality {
        self.quality
    }

    pub fn quality_settings(&self) -> QualitySettings {
        self.settings
    }

    /// Switches the preset: rebuilds pipelines and targets, keeps meshes and look.
    #[allow(dead_code)] // wired to the Settings menu by the game layer
    pub fn set_quality(&mut self, quality: Quality) {
        if quality == self.quality {
            return;
        }
        let mut fresh = Renderer::with_quality(self.device.clone(), self.queue.clone(), self.output_format, self.width, self.height, quality);
        fresh.meshes = std::mem::take(&mut self.meshes);
        fresh.free_slots = std::mem::take(&mut self.free_slots);
        fresh.scope = self.scope.take();
        fresh.statics = std::mem::take(&mut self.statics);
        fresh.static_cells = std::mem::take(&mut self.static_cells);
        fresh.look = self.look.clone();
        fresh.frame_budget = self.frame_budget;
        *self = fresh;
        log::info!("quality: {quality:?} {:?}", self.settings);
    }

    /// Sandstorm intensity 0..1: thick warm dust, short visibility, a
    /// muted sun, soft shadows, an amber sky. Effects add dust sheets and
    /// wipe ruts faster (the world renderer reads it from the look).
    pub fn set_storm(&mut self, intensity: f32) {
        self.look.storm = intensity.clamp(0.0, 1.0);
    }

    /// Feeds the last frame's wall time; adjusts the render scale (between
    /// the preset's minimum and maximum) to hold `frame_budget`. With GPU
    /// timestamps it steers by the GPU's own time (a vsynced frame time
    /// says nothing about headroom); without, by the frame time.
    #[allow(dead_code)] // wired to the Settings menu by the game layer
    pub fn note_frame_time(&mut self, dt: f32) {
        if !(0.0..1.0).contains(&dt) {
            return;
        }
        self.frame_avg += (dt - self.frame_avg) * 0.05;
        self.scale_cooldown -= dt;
        if self.scale_cooldown > 0.0 {
            return;
        }
        let (min, max) = (self.settings.min_scale, self.settings.render_scale);
        let mut next = self.scale;
        let budget_ms = self.frame_budget * 1000.0;
        if self.timer.is_some() {
            // Only on a fresh average, taken at the current scale.
            if !self.gpu_fresh {
                return;
            }
            self.gpu_fresh = false;
            let gpu = self.gpu_ms[0];
            if gpu > budget_ms * 0.85 {
                next = (self.scale - 0.1).max(min);
            } else {
                let up = (self.scale + 0.05).min(max);
                // Pixels scale with the square: step up only with room.
                if gpu * (up / self.scale).powi(2) < budget_ms * 0.7 {
                    next = up;
                }
            }
        } else if self.frame_avg > self.frame_budget * 1.12 {
            next = (self.scale - 0.1).max(min);
        } else if self.frame_avg < self.frame_budget * 0.8 {
            next = (self.scale + 0.05).min(max);
        }
        if (next - self.scale).abs() > 1e-3 {
            self.scale = next;
            let (iw, ih) = scaled(self.width, self.height, self.scale);
            self.targets = make_targets(&self.device, &self.post_layout, &self.post_uniform, &mut self.bloom, iw, ih, self.settings.msaa, self.fxaa.as_ref(), self.half_fx.as_ref());
            log::info!("render scale {:.2} (frame {:.1} ms, gpu {:.1} ms)", self.scale, self.frame_avg * 1000.0, self.gpu_ms[0]);
            if let Some(t) = &mut self.timer {
                t.sums = [0.0; 4];
                t.count = 0;
            }
        }
        self.scale_cooldown = 1.0;
    }

    #[allow(dead_code)] // wired to the Settings menu by the game layer
    pub fn render_scale(&self) -> f32 {
        self.scale
    }

    /// Bounding sphere and GPU row of an instance; `None` for missing or
    /// empty meshes.
    fn place(&self, inst: &Instance) -> Option<Placed> {
        let mesh = self.meshes.get(inst.mesh.0 as usize)?.as_ref()?;
        if mesh.count == 0 {
            return None;
        }
        let m = &inst.model;
        let scale = m.x_axis.truncate().length_squared().max(m.y_axis.truncate().length_squared()).max(m.z_axis.truncate().length_squared()).sqrt();
        Some(Placed {
            mesh: inst.mesh,
            centre: m.transform_point3(mesh.centre),
            radius: mesh.radius * scale,
            gpu: GpuInstance { model: m.to_cols_array_2d(), params: inst.params.to_array(), tint: inst.tint.to_array() },
        })
    }

    /// Sets the static scene (terrain, obstacles, dressing): placed once
    /// here, then only culled and packed each frame. Its meshes must stay
    /// alive until the next call.
    pub fn set_statics(&mut self, statics: &[Instance]) {
        let mut placed: Vec<Placed> = statics.iter().filter_map(|i| self.place(i)).collect();
        // Grouped by map cell, so a view can skip a whole cell at once.
        let cell_of = |p: &Placed| ((p.centre.x / STATIC_CELL).floor() as i32, (p.centre.z / STATIC_CELL).floor() as i32);
        placed.sort_by_key(|p| (cell_of(p), p.mesh));
        self.static_cells.clear();
        let mut i = 0;
        while i < placed.len() {
            let key = cell_of(&placed[i]);
            let start = i;
            while i < placed.len() && cell_of(&placed[i]) == key {
                i += 1;
            }
            let group = &placed[start..i];
            let centre = group.iter().map(|p| p.centre).sum::<Vec3>() / group.len() as f32;
            let radius = group.iter().map(|p| p.centre.distance(centre) + p.radius).fold(0.0, f32::max);
            self.static_cells.push((centre, radius, start as u32, i as u32));
        }
        self.statics = placed;
    }

    pub fn add_mesh(&mut self, m: &MeshBuilder) -> MeshId {
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for v in &m.vertices {
            lo = lo.min(Vec3::from(v.position));
            hi = hi.max(Vec3::from(v.position));
        }
        let (centre, radius) = if m.vertices.is_empty() { (Vec3::ZERO, 0.0) } else { ((lo + hi) * 0.5, (hi - lo).length() * 0.5) };
        let empty = m.indices.is_empty();
        let verts: &[u8] = if empty { &[0u8; std::mem::size_of::<crate::meshkit::Vertex>()] } else { bytemuck::cast_slice(&m.vertices) };
        let idx: &[u8] = if empty { &[0u8; 12] } else { bytemuck::cast_slice(&m.indices) };
        let mesh = GpuMesh {
            vertices: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("mesh vertices"), contents: verts, usage: wgpu::BufferUsages::VERTEX }),
            indices: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh indices"),
                contents: idx,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            }),
            count: m.indices.len() as u32,
            centre,
            radius,
        };
        let id = match self.free_slots.pop() {
            Some(slot) => {
                self.meshes[slot as usize] = Some(mesh);
                MeshId(slot)
            }
            None => {
                self.meshes.push(Some(mesh));
                MeshId(self.meshes.len() as u32 - 1)
            }
        };
        if let Some(scope) = &mut self.scope {
            scope.push(id);
        }
        if let Some(rec) = &mut self.recorded {
            if m.vertices.len() <= RECORD_MAX_VERTS {
                rec.insert(id, m.clone());
            }
        }
        id
    }

    /// Whether `id` was uploaded in the open scope.
    pub fn in_open_scope(&self, id: MeshId) -> bool {
        self.scope.as_ref().is_some_and(|s| s.contains(&id))
    }

    /// Overwrites part of a mesh's index buffer (`first` in indices): merged
    /// chunks blank a destroyed piece's triangles this way, and restore them.
    pub fn write_indices(&self, id: MeshId, first: u32, indices: &[u32]) {
        if let Some(m) = self.meshes.get(id.0 as usize).and_then(|m| m.as_ref()) {
            if !indices.is_empty() && first as usize + indices.len() <= m.count as usize {
                self.queue.write_buffer(&m.indices, first as u64 * 4, bytemuck::cast_slice(indices));
            }
        }
    }

    /// Frees a mesh uploaded in the open scope and forgets it there (so the
    /// scope's end does not free its slot again once reused).
    pub fn free_scoped(&mut self, id: MeshId) {
        if let Some(scope) = &mut self.scope {
            scope.retain(|m| *m != id);
        }
        if let Some(rec) = &mut self.recorded {
            rec.remove(&id);
        }
        self.free_mesh(id);
    }

    /// Radius of a mesh's bounding sphere (0 for missing meshes).
    pub fn mesh_radius(&self, id: MeshId) -> f32 {
        self.meshes.get(id.0 as usize).and_then(|m| m.as_ref()).map_or(0.0, |m| m.radius)
    }

    /// Like `add_mesh`, but never recorded in an open scope (for meshes a
    /// cache keeps across scenes).
    pub fn add_mesh_pinned(&mut self, m: &MeshBuilder) -> MeshId {
        let scope = self.scope.take();
        let id = self.add_mesh(m);
        self.scope = scope;
        id
    }

    /// Frees a mesh's GPU buffers; its id may be reused by a later upload.
    pub fn free_mesh(&mut self, id: MeshId) {
        if let Some(slot) = self.meshes.get_mut(id.0 as usize) {
            if slot.take().is_some() {
                self.free_slots.push(id.0);
            }
        }
    }

    /// Starts recording uploaded meshes (a map, a line-up, a podium).
    pub fn begin_scope(&mut self) {
        self.scope = Some(Vec::new());
    }

    pub fn in_scope(&self) -> bool {
        self.scope.is_some()
    }

    /// Stops recording and returns what was uploaded since `begin_scope`.
    pub fn end_scope(&mut self) -> Vec<MeshId> {
        self.scope.take().unwrap_or_default()
    }

    /// The output size, pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn mesh_count(&self) -> usize {
        self.meshes.iter().filter(|m| m.is_some()).count()
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) == (self.width, self.height) {
            return;
        }
        self.width = width;
        self.height = height;
        let (iw, ih) = scaled(width, height, self.scale);
        self.targets = make_targets(&self.device, &self.post_layout, &self.post_uniform, &mut self.bloom, iw, ih, self.settings.msaa, self.fxaa.as_ref(), self.half_fx.as_ref());
    }

    /// Cascade far distances: the look's, or with two cascades (Low) a
    /// near one a little longer and the second reaching about half as far.
    fn splits(&self) -> [f32; 3] {
        let s = self.look.splits;
        if self.settings.cascades >= 3 {
            s
        } else {
            let second = s[2] * 0.55;
            [s[0] * 1.5, second, second]
        }
    }

    /// Fits each cascade round its slice of the view, snapped to texels.
    fn cascades(&self, camera: &Camera, aspect: f32) -> [(Mat4, Mat4, f32); CASCADES] {
        let sun = self.look.sun_direction();
        let fwd = (camera.target - camera.eye).normalize();
        let tan = (camera.fov_y * 0.5).tan();
        let rot = look_at_mat4(Vec3::ZERO, -sun, Vec3::Y);
        let inv_rot = rot.inverse();
        let mut near = 0.1;
        let splits = self.splits();
        let mut out = [(Mat4::IDENTITY, Mat4::IDENTITY, 0.0); CASCADES];
        for (i, slot) in out.iter_mut().enumerate() {
            let far = splits[i];
            // Bounding sphere of the slice: centred along the view axis.
            let h_far = far * tan;
            let w_far = h_far * aspect;
            let mid = 0.5 * (near + far);
            let corner_far = Vec3::new(w_far, h_far, far);
            let radius = ((corner_far - Vec3::new(0.0, 0.0, mid)).length()).max(far * 0.55);
            let centre = camera.eye + fwd * mid;
            let texel = 2.0 * radius / self.settings.shadow_size as f32;
            let ls = rot.transform_point3(centre);
            let snapped = Vec3::new((ls.x / texel).round() * texel, (ls.y / texel).round() * texel, ls.z);
            let centre = inv_rot.transform_point3(snapped);
            let depth = 2.0 * radius + SHADOW_BACK;
            let view = look_at_mat4(centre + sun * (radius + SHADOW_BACK), centre, Vec3::Y);
            let projm = proj::orthographic(-radius, radius, -radius, radius, 0.0, depth);
            *slot = (projm * view, view, radius);
            near = far;
        }
        out
    }

    /// Draws one frame into `output`.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        output: &wgpu::TextureView,
        camera: &Camera,
        time: f32,
        instances: &[Instance],
        sprites: &[Sprite],
        ruts: &[Rut],
        blobs: &[Vec4],
        lights: &[Light],
        occluders: &[Vec4],
    ) {
        self.look.sun_dir = self.look.sun_direction();
        let aspect = self.width as f32 / self.height as f32;
        // GPU timing: read what finished, pick a slot for this frame.
        if let Some(timer) = &mut self.timer {
            let _ = self.device.poll(wgpu::PollType::Poll);
            if let Some(avg) = timer.collect() {
                self.gpu_ms = avg;
                self.gpu_fresh = true;
                // At most every five seconds.
                let now = crate::world::clock_ms();
                if now - self.gpu_logged > 5000.0 {
                    self.gpu_logged = now;
                    log::info!(
                        "gpu {:?} {}x{} scale {:.2}: {:.2} ms (shadows {:.2}, scene {:.2}, bloom {:.2}, post {:.2})",
                        self.quality,
                        self.width,
                        self.height,
                        self.scale,
                        avg[0],
                        avg[1],
                        avg[2],
                        avg[3],
                        avg[4]
                    );
                }
            }
            timer.begin();
        }
        // The cascades redrawn this frame: every one in use, the farthest
        // only every `far_shadow_every` frames (it shifts least on screen);
        // a cascade not redrawn keeps the matrix it was drawn with.
        let fresh = self.cascades(camera, aspect);
        let used = self.settings.cascades.clamp(1, CASCADES);
        let mut redraw = [false; CASCADES];
        for (i, r) in redraw.iter_mut().enumerate().take(used) {
            let far = used > 1 && i + 1 == used;
            *r = !far || self.frames < 2 || self.frames % u64::from(self.settings.far_shadow_every.max(1)) == 0;
            if *r {
                self.cascade_state[i] = fresh[i];
            }
        }
        self.frames += 1;
        let cascades = self.cascade_state;
        let splits = self.splits();
        let look = &self.look;
        let view = look_at_mat4(camera.eye, camera.target, Vec3::Y);
        let projm = proj::perspective(camera.fov_y, aspect, 0.1, 3000.0);
        let view_proj = projm * view;
        let inv_view = view.inverse();
        let right = inv_view.x_axis.truncate();
        let up = inv_view.y_axis.truncate();

        let mut occ = [[0.0f32; 4]; MAX_OCCLUDERS];
        // Nearest occluders to the eye first.
        let mut sorted: Vec<Vec4> = occluders.to_vec();
        sorted.sort_by(|a, b| a.truncate().distance_squared(camera.eye).partial_cmp(&b.truncate().distance_squared(camera.eye)).unwrap_or(std::cmp::Ordering::Equal));
        for (slot, o) in occ.iter_mut().zip(sorted.iter()) {
            *slot = o.to_array();
        }
        let mut light_pos = [[0.0f32; 4]; MAX_LIGHTS];
        let mut light_col = [[0.0f32; 4]; MAX_LIGHTS];
        let max_lights = self.settings.max_lights.min(MAX_LIGHTS);
        for (i, l) in lights.iter().take(max_lights).enumerate() {
            light_pos[i] = l.pos.extend(l.radius).to_array();
            light_col[i] = l.color.extend(0.0).to_array();
        }
        let shadow_size = self.settings.shadow_size as f32;
        let texel = |r: f32| 2.0 * r / shadow_size;
        // The storm bends the look: everything below reads `l`.
        let s = look.storm;
        let storm_dust = Vec3::new(0.78, 0.5, 0.3);
        let mut l = look.clone();
        if s > 0.0 {
            l.haze = look.haze.lerp(storm_dust * 1.1, s);
            l.haze_start = look.haze_start + (4.0 - look.haze_start) * s;
            l.haze_density = look.haze_density * (1.0 + 30.0 * s * s) + 0.004 * s;
            l.height_fog = look.height_fog * (1.0 + 10.0 * s);
            l.height_falloff = look.height_falloff * (1.0 - 0.6 * s);
            l.sun_color = look.sun_color.lerp(Vec3::new(1.0, 0.62, 0.38) * 1.2, s);
            l.sky_color = look.sky_color.lerp(storm_dust * 0.7, s);
            l.ground_color = look.ground_color.lerp(storm_dust * 0.8, s * 0.6);
            l.air_glow = look.air_glow * (1.0 + 2.0 * s);
        }
        if self.salt_flat {
            // Strongest at noon, nothing at golden hour or dusk.
            let k = ((look.sun_elevation - 25.0) / 40.0).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            l.exposure = look.exposure * (1.0 - 0.12 * k);
            l.saturation = look.saturation * (1.0 + 0.22 * k);
            l.warmth = look.warmth * Vec3::new(1.0 + 0.04 * k, 1.0, 1.0 - 0.07 * k);
            l.sun_color = look.sun_color * Vec3::new(1.0, 1.0 - 0.03 * k, 1.0 - 0.1 * k);
        }
        if self.gorge {
            // Shaded canyon walls read as rock, not as black: more sky and
            // much more warm bounce (the sunlit walls light the gorge).
            l.sky_color = l.sky_color * 1.35;
            l.ground_color = l.ground_color * 1.6;
        }
        let look = &l;
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            cascades: [cascades[0].0.to_cols_array_2d(), cascades[1].0.to_cols_array_2d(), cascades[2].0.to_cols_array_2d()],
            camera: camera.eye.extend(time).to_array(),
            // w: a pixel's size (m) at 1 m from the eye, at the render
            // scale (thin wires and cables keep a minimum screen width).
            cam_right: right.extend(2.0 * (camera.fov_y * 0.5).tan() / scaled(self.width, self.height, self.scale).1 as f32).to_array(),
            cam_up: up.extend(0.0).to_array(),
            sun_dir: look.sun_dir.normalize().extend(0.0).to_array(),
            sun_color: look.sun_color.extend(0.0).to_array(),
            sky_color: look.sky_color.extend(0.0).to_array(),
            ground_color: look.ground_color.extend(0.0).to_array(),
            fog: look.haze.extend(look.haze_start).to_array(),
            fog2: [look.haze_density, look.height_fog, look.height_falloff, look.mie_g],
            shadow: [shadow_size, look.shadow_softness, look.air_glow, look.storm],
            splits: [splits[0], splits[1], splits[2], used as f32],
            texels: [texel(cascades[0].2), texel(cascades[1].2), texel(cascades[2].2), if self.settings.lite { 1.0 } else { 0.0 }],
            // w: the blue sky's gain, so it reads the same at any exposure.
            ranges: [2.0 * cascades[0].2 + SHADOW_BACK, 2.0 * cascades[1].2 + SHADOW_BACK, 2.0 * cascades[2].2 + SHADOW_BACK, look.sky_gain / look.exposure.max(0.05)],
            misc: [occluders.len().min(MAX_OCCLUDERS) as f32, lights.len().min(max_lights) as f32, look.rim, look.spec],
            occluders: occ,
            light_pos,
            light_col,
            focus: self.focus.extend(self.focus_strength).to_array(),
        };
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        for (i, (buf, _)) in self.cascade_binds.iter().enumerate() {
            self.queue.write_buffer(buf, 0, bytemuck::bytes_of(&cascades[i].0.to_cols_array_2d()));
        }
        // The tonemap draws at the render scale into the LDR target when
        // FXAA follows (which then upscales), else straight to the output.
        let (iw, ih) = scaled(self.width, self.height, self.scale);
        let (pw, ph) = if self.fxaa.is_some() { (iw, ih) } else { (self.width, self.height) };
        let encode = self.encode_srgb || self.fxaa.is_some();
        let post = [
            Vec4::new(look.exposure, look.vignette, if encode { 1.0 } else { 0.0 }, look.curve as f32),
            look.warmth.extend(look.saturation),
            Vec4::new(look.bloom, look.contrast, 1.0 / pw as f32, 1.0 / ph as f32),
        ];
        self.queue.write_buffer(&self.post_uniform, 0, bytemuck::cast_slice(&post));
        if let Some(f) = &self.fxaa {
            let u = [1.0 / iw as f32, 1.0 / ih as f32, if self.encode_srgb { 0.0 } else { 1.0 }, 0.0, 1.0 / self.width as f32, 1.0 / self.height as f32, 0.0, 0.0];
            self.queue.write_buffer(&f.uniform, 0, bytemuck::cast_slice(&u));
        }

        // Cull per view, sort by mesh, pack all views into one buffer.
        let cull_start = crate::world::clock_ms();
        let mut moving = std::mem::take(&mut self.moving);
        moving.clear();
        moving.extend(instances.iter().filter_map(|i| self.place(i)));
        let planes = frustum_planes(view_proj);
        // Pixels per metre at unit distance, for the on-screen size test.
        let focal = self.height as f32 * 0.5 / (camera.fov_y * 0.5).tan();
        let (min_screen, shadow_lod) = (self.settings.min_screen_radius, self.settings.shadow_lod);
        let eye = camera.eye;
        let statics = &self.statics;
        let total = statics.len() + moving.len();
        let mut packed: Vec<GpuInstance> = Vec::with_capacity(total * 2);
        let mut batches: Vec<Vec<(MeshId, u32, u32)>> = Vec::with_capacity(CASCADES + 1);
        // (mesh, index into statics, or statics.len() + index into moving).
        let mut visible: Vec<(MeshId, u32)> = Vec::with_capacity(total);
        for view in 0..=CASCADES {
            visible.clear();
            if view > 0 && !redraw[view - 1] {
                batches.push(Vec::new());
                continue;
            }
            let light = (view > 0).then(|| (cascades[view - 1].1, cascades[view - 1].2));
            let inside = |c: Vec3, r: f32| -> bool {
                match light {
                    None => sphere_in_planes(&planes, c, r),
                    Some((lv, radius)) => {
                        let q = lv.transform_point3(c);
                        q.x.abs() < radius + r && q.y.abs() < radius + r && -q.z > -r && -q.z < 2.0 * radius + SHADOW_BACK + r
                    }
                }
            };
            let keep = |p: &Placed| -> bool {
                let (c, r) = (p.centre, p.radius);
                let d = c.distance(eye);
                // The camera skips what is tiny on screen; shadows skip
                // small casters far from the eye.
                let big_enough = if light.is_none() { r * focal >= min_screen * d } else { r * shadow_lod >= d };
                big_enough && inside(c, r)
            };
            for &(c, r, start, end) in &self.static_cells {
                if !inside(c, r) {
                    continue;
                }
                for k in start..end {
                    let p = &statics[k as usize];
                    if keep(p) {
                        visible.push((p.mesh, k));
                    }
                }
            }
            for (k, p) in moving.iter().enumerate() {
                if keep(p) {
                    visible.push((p.mesh, (statics.len() + k) as u32));
                }
            }
            visible.sort_unstable_by_key(|v| (v.0, v.1));
            let mut list = Vec::new();
            let mut i = 0;
            while i < visible.len() {
                let mesh = visible[i].0;
                let first = packed.len() as u32;
                while i < visible.len() && visible[i].0 == mesh {
                    let k = visible[i].1 as usize;
                    packed.push(if k < statics.len() { statics[k].gpu } else { moving[k - statics.len()].gpu });
                    i += 1;
                }
                list.push((mesh, first, packed.len() as u32 - first));
            }
            batches.push(list);
        }
        self.moving = moving;
        self.cull_ms = crate::world::clock_ms() - cull_start;
        self.sprite_count = sprites.len() as u32;
        self.instances.write(&self.device, &self.queue, bytemuck::cast_slice(&packed));
        self.sprites.write(&self.device, &self.queue, bytemuck::cast_slice(sprites));
        self.ruts.write(&self.device, &self.queue, bytemuck::cast_slice(ruts));
        let range = self.settings.blob_range;
        let near_blobs: Vec<Vec4> = blobs.iter().copied().filter(|b| b.truncate().distance(camera.eye) < range).collect();
        self.blobs.write(&self.device, &self.queue, bytemuck::cast_slice(&near_blobs));
        self.stats = (batches.iter().map(|b| b.len() as u32).sum(), packed.len() as u32);

        let t_upload = crate::world::clock_ms();
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        let timer = self.timer.as_ref();
        let half_fx = self.half_fx.as_ref().zip(self.targets.fx.as_ref());
        for (i, layer) in self.shadow_layers.iter().enumerate() {
            if !redraw[i] {
                continue;
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: layer,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: if i == 0 { timer.and_then(|t| t.writes(Some(0), None)) } else { None },
                ..Default::default()
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(1, &self.cascade_binds[i].1, &[]);
            self.draw_batches(&mut pass, &batches[i + 1]);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.targets.msaa_color.as_ref().unwrap_or(&self.targets.hdr),
                    depth_slice: None,
                    resolve_target: self.targets.msaa_color.as_ref().map(|_| &self.targets.hdr),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: if self.settings.msaa > 1 { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.targets.depth,
                    // Kept when the half-resolution effects pass reads it.
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: if half_fx.is_some() { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard } }),
                    stencil_ops: None,
                }),
                timestamp_writes: timer.and_then(|t| t.writes(Some(1), if half_fx.is_some() { None } else { Some(2) })),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.scene_bind, &[]);
            pass.set_pipeline(&self.scene_pipeline);
            self.draw_batches(&mut pass, &batches[0]);
            pass.set_pipeline(&self.background_pipeline);
            pass.draw(0..3, 0..1);
            if !near_blobs.is_empty() {
                pass.set_pipeline(&self.blob_pipeline);
                pass.set_vertex_buffer(0, self.blobs.buffer.slice(..));
                pass.draw(0..6, 0..near_blobs.len() as u32);
            }
            if !ruts.is_empty() {
                pass.set_pipeline(&self.rut_pipeline);
                pass.set_vertex_buffer(0, self.ruts.buffer.slice(..));
                pass.draw(0..6, 0..ruts.len() as u32);
            }
            self.glass.draw(&mut pass);
            if !sprites.is_empty() && half_fx.is_none() {
                pass.set_pipeline(&self.sprite_pipeline);
                pass.set_vertex_buffer(0, self.sprites.buffer.slice(..));
                pass.draw(0..6, 0..sprites.len() as u32);
            }
        }
        // Half resolution effects: smoke, dust and fire into their own
        // target, then laid over the frame (before bloom, so fire glows).
        if let Some((h, fx)) = half_fx {
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("half fx pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &fx.color,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), store: wgpu::StoreOp::Store },
                    })],
                    ..Default::default()
                });
                if !sprites.is_empty() {
                    pass.set_pipeline(&h.pipeline);
                    pass.set_bind_group(0, &self.scene_bind, &[]);
                    pass.set_bind_group(1, &fx.depth_bind, &[]);
                    pass.set_vertex_buffer(0, self.sprites.buffer.slice(..));
                    pass.draw(0..6, 0..sprites.len() as u32);
                }
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fx mix pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets.hdr,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                timestamp_writes: timer.and_then(|t| t.writes(None, Some(2))),
                ..Default::default()
            });
            pass.set_pipeline(&h.mix_pipeline);
            pass.set_bind_group(0, &fx.mix_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.bloom.record(&self.queue, &mut encoder, self.look.bloom_threshold, 0.5, self.look.exposure, self.look.bloom_scatter);
        let fxaa = self.fxaa.as_ref().zip(self.targets.fxaa_bind.as_ref()).zip(self.targets.ldr.as_ref());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("post pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: fxaa.map_or(output, |(_, ldr)| ldr),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                timestamp_writes: timer.and_then(|t| t.writes(Some(3), if fxaa.is_some() { None } else { Some(4) })),
                ..Default::default()
            });
            pass.set_pipeline(&self.post_pipeline);
            pass.set_bind_group(0, &self.targets.post_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(((f, bind), _)) = fxaa {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fxaa pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                timestamp_writes: timer.and_then(|t| t.writes(None, Some(4))),
                ..Default::default()
            });
            pass.set_pipeline(&f.pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(t) = timer {
            t.resolve(&mut encoder);
        }
        let t_encode = crate::world::clock_ms();
        self.queue.submit(std::iter::once(encoder.finish()));
        if let Some(t) = &mut self.timer {
            t.after_submit();
        }
        let t_end = crate::world::clock_ms();
        self.phase_ms = [t_upload - cull_start - self.cull_ms, t_encode - t_upload, t_end - t_encode];
    }

    fn draw_batches(&self, pass: &mut wgpu::RenderPass<'_>, batches: &[(MeshId, u32, u32)]) {
        pass.set_vertex_buffer(1, self.instances.buffer.slice(..));
        for &(mesh, first, count) in batches {
            let Some(m) = self.meshes[mesh.0 as usize].as_ref() else { continue };
            pass.set_vertex_buffer(0, m.vertices.slice(..));
            pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..m.count, 0, first..first + count);
        }
    }
}

fn frustum_planes(m: Mat4) -> [Vec4; 6] {
    let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
    let norm = |p: Vec4| p / p.truncate().length();
    [norm(r3 + r0), norm(r3 - r0), norm(r3 + r1), norm(r3 - r1), norm(r2), norm(r3 - r2)]
}

fn sphere_in_planes(planes: &[Vec4; 6], c: Vec3, r: f32) -> bool {
    planes.iter().all(|p| p.truncate().dot(c) + p.w > -r)
}

fn scaled(w: u32, h: u32, scale: f32) -> (u32, u32) {
    (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1))
}

#[allow(clippy::too_many_arguments)]
fn make_targets(device: &wgpu::Device, post_layout: &wgpu::BindGroupLayout, post_uniform: &wgpu::Buffer, bloom: &mut Bloom, width: u32, height: u32, msaa: u32, fxaa: Option<&FxaaPass>, half: Option<&HalfFx>) -> Targets {
    let make = |label, format, samples, usage| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    };
    let msaa_color = (msaa > 1).then(|| make("msaa color", HDR_FORMAT, msaa, wgpu::TextureUsages::RENDER_ATTACHMENT));
    // The half-resolution effects pass reads the scene depth.
    let depth_usage = if half.is_some() { wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING } else { wgpu::TextureUsages::RENDER_ATTACHMENT };
    let depth = make("depth", DEPTH_FORMAT, msaa, depth_usage);
    let hdr = make("hdr", HDR_FORMAT, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
    bloom.resize(device, &hdr, width, height);
    let post_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("post bind"),
        layout: post_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: post_uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&hdr) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(bloom.output()) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(bloom.sampler()) },
        ],
    });
    let ldr = fxaa.map(|_| make("ldr", LDR_FORMAT, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING));
    let fxaa_bind = fxaa.zip(ldr.as_ref()).map(|(f, view)| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fxaa bind"),
            layout: &f.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: f.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(bloom.sampler()) },
            ],
        })
    });
    let fx = half.map(|h| {
        let color = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("half fx"),
                size: wgpu::Extent3d { width: width.div_ceil(2), height: height.div_ceil(2), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fx depth bind"),
            layout: &h.depth_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&depth) }],
        });
        let mix_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fx mix bind"),
            layout: &h.mix_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&color) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(bloom.sampler()) },
            ],
        });
        FxTargets { color, depth_bind, mix_bind }
    });
    Targets { msaa_color, depth, hdr, post_bind, ldr, fxaa_bind, fx }
}

/// Whether the page URL's query has `kv` (web only; always false natively).
fn url_has(kv: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().search().ok())
            .is_some_and(|s| s.trim_start_matches('?').split('&').any(|x| x == kv))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = kv;
        false
    }
}

/// Requests a device with WebGPU-portable limits.
pub async fn request_device(adapter: &wgpu::Adapter) -> anyhow::Result<(wgpu::Device, wgpu::Queue)> {
    let info = adapter.get_info();
    log::info!("adapter: {} ({:?}, {:?})", info.name, info.backend, info.device_type);
    let limits = wgpu::Limits::default().using_resolution(adapter.limits());
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("tansk device"),
            // GPU timing where offered (WebGPU's `timestamp-query`).
            required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
            required_limits: limits,
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::anyhow!("requesting a wgpu device: {e}"))?;
    Ok((device, queue))
}

#[cfg(test)]
mod look_calib {
    //! Exposure calibration: every time of day on both maps, plus the storm,
    //! rendered to `TANSK_LOOK_DIR` (default `docs/shots/look`). Run with
    //! `cargo test -p arena-client --release -- --ignored shots_look`, then
    //! check the luminance histograms against the references. `gpu_bench`
    //! times each preset at 1080p with timestamp queries (where supported).
    use super::*;
    use crate::world::{paint_for, RenderWorld, WorldRenderer};
    use arena_core::{Map, MapId, Sim};

    fn device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::PRIMARY, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() })).expect("adapter");
        pollster::block_on(request_device(&adapter)).expect("device")
    }

    /// A practice match on `map` played `seconds`, effects fed for the last
    /// stretch; the camera behind a live tank, or at `eye` looking at `look`.
    fn scene(wr: &mut WorldRenderer, map: MapId, seconds: f32, view: Option<(Vec3, Vec3)>) -> RenderWorld {
        let seed = 7;
        let (mut sim, _) = Sim::practice_on(map, seed, 7, 0.7);
        wr.set_map(&Map::by_id(map, seed));
        let mut world = RenderWorld::default();
        let ticks = (seconds * 60.0) as u32;
        for tick in 0..ticks {
            sim.step();
            let ev = sim.drain_events();
            if tick + 600 > ticks {
                world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
                world.time = tick as f32 * arena_core::DT;
                let (eye, at) = view.unwrap_or_else(|| {
                    let t = world.tanks.iter().find(|t| t.alive).map(|t| (t.pos, t.rot)).unwrap_or((Vec3::ZERO, glam::Quat::IDENTITY));
                    let fwd = t.1 * Vec3::X;
                    (t.0 - fwd * 10.0 + Vec3::Y * 4.0, t.0 + fwd * 25.0 + Vec3::Y * 1.5)
                });
                world.camera = Some(Camera::look(eye, at, 58.0));
                for e in &ev {
                    wr.event(e, eye);
                }
                wr.fx.update(arena_core::DT, &world);
            }
        }
        world.dt = 0.0;
        world
    }

    fn save(wr: &mut WorldRenderer, world: &RenderWorld, w: u32, h: u32, path: &str) {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let r = &wr.renderer;
        let texture = r.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        // A few frames, so every-other-frame work (far shadows) is settled.
        for _ in 0..3 {
            wr.draw(&view, world);
        }
        let r = &wr.renderer;
        let padded = (w * 4).div_ceil(256) * 256;
        let buffer = r.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (padded * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut enc = r.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        r.queue.submit([enc.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        r.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let mapped = buffer.slice(..).get_mapped_range().expect("mapped");
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h as usize {
            px.extend_from_slice(&mapped[row * padded as usize..row * padded as usize + (w * 4) as usize]);
        }
        image::save_buffer(path, &px, w, h, image::ExtendedColorType::Rgba8).expect("save");
    }

    /// The FXAA shader parses and validates (the others are checked in
    /// `world::shots::shaders_validate`).
    #[test]
    fn fxaa_validates() {
        use wgpu::naga;
        let src = include_str!("shaders/fxaa.wgsl");
        let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("fxaa: {}", e.emit_to_string(src)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&module).expect("fxaa validates");
    }

    /// Medium is the 60 FPS preset: no MSAA, post AA, the far cascade
    /// every other frame, a render scale below one.
    #[test]
    fn presets_scale_down() {
        let (h, m, l) = (Quality::High.settings(), Quality::Medium.settings(), Quality::Low.settings());
        assert!(m.fxaa && m.msaa == 1 && m.far_shadow_every > 1 && m.render_scale < 1.0);
        assert!(l.fxaa && l.lite && l.cascades == 2);
        assert!(h.bloom_levels > m.bloom_levels && m.bloom_levels > l.bloom_levels);
        assert!(l.min_scale <= m.min_scale && m.min_scale <= h.min_scale);
        assert!(h.fxaa && h.msaa == 1 && Quality::Ultra.settings().msaa == 4);
    }

    /// See-through: a tank parked behind a container, seen from the far
    /// side as a chase camera would after the tank slips behind it; off
    /// then on (`docs/shots/see-through-off.png`, `-on.png`).
    #[test]
    #[ignore]
    fn shots_see_through() {
        let dir = format!("{}/../../docs/shots", env!("CARGO_MANIFEST_DIR"));
        let (w, h) = (1600, 900);
        let (device, queue) = device();
        let mut wr = WorldRenderer::new(Renderer::with_quality(device, queue, wgpu::TextureFormat::Rgba8Unorm, w, h, Quality::Medium));
        wr.begin_scene();
        let floor = wr.renderer.add_mesh(&crate::terrain::flat(200.0));
        let mut box_mesh = crate::props::container(Vec3::new(6.1, 1.3, 1.22), crate::props::CONTAINER_COLOURS[1]);
        box_mesh.transform_from(0, Mat4::from_translation(Vec3::Y * 1.3));
        let container = wr.renderer.add_mesh(&box_mesh);
        let mut shack = crate::props::container(Vec3::new(3.0, 1.3, 1.22), crate::props::CONTAINER_COLOURS[0]);
        shack.transform_from(0, Mat4::from_translation(Vec3::Y * 1.3));
        let shack = wr.renderer.add_mesh(&shack);
        wr.set_statics(vec![
            Instance::new(floor, Mat4::IDENTITY),
            Instance::new(container, Mat4::IDENTITY),
            Instance::new(shack, Mat4::from_translation(Vec3::new(-9.0, 0.0, -14.0))),
        ]);
        let mut world = RenderWorld::default();
        world.tanks.push(crate::world::parked_tank(1, crate::tank::RUSTBACK, Vec3::new(0.5, 0.0, -5.5), 0.3, arena_core::PartKind::HeavyCannon, vec![]));
        world.camera = Some(Camera::look(Vec3::new(1.5, 3.3, 7.5), Vec3::new(0.5, 1.2, -18.0), 58.0));
        for (on, name) in [(false, "off"), (true, "on")] {
            wr.renderer.see_through_on = on;
            save(&mut wr, &world, w, h, &format!("{dir}/see-through-{name}.png"));
        }
        // Close behind the tank: its own hull must never stipple.
        world.camera = Some(Camera::look(Vec3::new(-5.0, 2.6, -3.6), Vec3::new(4.0, 1.0, -8.0), 62.0));
        save(&mut wr, &world, w, h, &format!("{dir}/see-through-close.png"));
    }

    /// See-through in play: Raider Waves at the Refinery outpost from a
    /// chase camera behind the human tank, turning round in steps, with
    /// see-through off and on (to `TANSK_LOOK_DIR`, default docs/shots/look).
    #[test]
    #[ignore]
    fn shots_see_through_raid() {
        let dir = std::env::var("TANSK_LOOK_DIR").unwrap_or_else(|_| format!("{}/../../docs/shots/look", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).expect("dir");
        let (w, h) = (1280, 720);
        let (device, queue) = device();
        let mut wr = WorldRenderer::new(Renderer::with_quality(device, queue, wgpu::TextureFormat::Rgba8Unorm, w, h, Quality::Medium));
        let (mut sim, me) = Sim::practice_raid_on(MapId::Refinery, 7, 3, 0.7);
        wr.set_map(&Map::by_id(MapId::Refinery, 7));
        let mut world = RenderWorld::default();
        for tick in 0..(60 * 20) {
            sim.step();
            let ev = sim.drain_events();
            if tick > 60 * 12 {
                world = RenderWorld::from_snapshot(&sim.snapshot(), &paint_for);
                world.time = tick as f32 * arena_core::DT;
                for e in &ev {
                    wr.event(e, Vec3::ZERO);
                }
                wr.fx.update(arena_core::DT, &world);
            }
        }
        world.dt = 0.0;
        let t = world.tanks.iter().find(|t| t.player == me).map(|t| t.pos).unwrap_or(Vec3::ZERO);
        for k in 0..6 {
            let a = k as f32 * std::f32::consts::TAU / 6.0;
            let back = Vec3::new(a.cos(), 0.0, a.sin());
            let eye = t - back * 7.5 + Vec3::Y * 3.6;
            world.camera = Some(Camera::look(eye, t + back * 30.0 + Vec3::Y * 1.0, 58.0));
            for (on, name) in [(false, "off"), (true, "on")] {
                wr.renderer.see_through_on = on;
                save(&mut wr, &world, w, h, &format!("{dir}/st-raid-{k}-{name}.png"));
            }
        }
    }

    /// Explosion stress at the Refinery, 1080p, Medium and Low: the
    /// effects' stress spawner keeps ~20 blasts, fuel-trailer explosions and
    /// burning wrecks in view. Prints the CPU split (effects update, sprite
    /// build, culling and upload, encode) and the GPU time (timestamp
    /// queries, frames drawn one at a time), and writes a frame per preset
    /// to `TANSK_LOOK_DIR`. `cargo test -p arena-client --release --
    /// --ignored gpu_bench_boom --nocapture`.
    #[test]
    #[ignore]
    fn gpu_bench_boom() {
        let (w, h) = (1920, 1080);
        let (device, queue) = device();
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let dir = std::env::var("TANSK_LOOK_DIR").ok();
        for (q, stress) in [(Quality::Medium, false), (Quality::Medium, true), (Quality::Low, false), (Quality::Low, true)] {
            let mut wr = WorldRenderer::new(Renderer::with_quality(device.clone(), queue.clone(), format, w, h, q));
            let mut world = scene(&mut wr, MapId::Refinery, 8.0, Some((Vec3::new(40.0, 9.0, 40.0), Vec3::new(110.0, 2.0, -20.0))));
            wr.fx.stress = stress;
            world.dt = 1.0 / 60.0;
            let (mut cpu, mut fx, mut n) = (0.0f64, [0.0f64; 4], 0);
            let mut gpu = [0.0f32; 5];
            let mut gn = 0;
            let mut parts = 0;
            for frame in 0..600 {
                world.time += world.dt;
                let t0 = std::time::Instant::now();
                wr.draw(&view, &world);
                let t1 = t0.elapsed().as_secs_f64() * 1000.0;
                // One at a time: clean GPU timings (and no runaway queue).
                wr.renderer.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
                if frame >= 240 {
                    cpu += t1;
                    for (s, v) in fx.iter_mut().zip(wr.prof) {
                        *s += v;
                    }
                    n += 1;
                    parts += wr.fx.particle_count();
                    if frame % 60 == 0 && wr.renderer.gpu_ms[0] > 0.0 {
                        for (s, v) in gpu.iter_mut().zip(wr.renderer.gpu_ms) {
                            *s += v;
                        }
                        gn += 1;
                    }
                }
            }
            let nf = n as f64;
            let g = gpu.map(|s| s / gn.max(1) as f32);
            println!(
                "BOOM {q:?} stress {stress}: {} particles, cpu draw {:.2} ms (fx {:.2}, inst {:.2}, sprites {:.2}, render {:.2}), cull {:.2}, upload {:.2}, encode {:.2} | gpu {:.2} ms (shadows {:.2}, scene+fx {:.2}, bloom {:.2}, post {:.2})",
                parts / n.max(1),
                cpu / nf,
                fx[0] / nf,
                fx[1] / nf,
                fx[2] / nf,
                fx[3] / nf,
                wr.renderer.cull_ms,
                wr.renderer.phase_ms[0],
                wr.renderer.phase_ms[1],
                g[0],
                g[1],
                g[2],
                g[3],
                g[4]
            );
            if let Some(d) = &dir {
                world.dt = 0.0;
                save(&mut wr, &world, w, h, &format!("{d}/boom-{q:?}-{stress}.png"));
            }
        }
    }

    type View = (&'static str, MapId, f32, Option<(Vec3, Vec3)>);
    const VIEWS: [View; 4] = [
        ("sink-play", MapId::Sink, 30.0, None),
        ("sink-wide", MapId::Sink, 30.0, Some((Vec3::new(-60.0, 26.0, 70.0), Vec3::new(10.0, 0.0, 0.0)))),
        ("refinery-play", MapId::Refinery, 30.0, None),
        ("refinery-farm", MapId::Refinery, 30.0, Some((Vec3::new(95.0, 30.0, -40.0), Vec3::new(180.0, 4.0, 40.0)))),
    ];

    /// The presets as they were before the 60 FPS pass (4x MSAA on Medium
    /// and High, three cascades every frame, six bloom levels), for
    /// `gpu_bench`'s before/after.
    fn legacy(q: Quality) -> QualitySettings {
        let mut s = q.settings();
        s.cascades = 3;
        s.far_shadow_every = 1;
        s.fxaa = false;
        s.bloom_levels = 6;
        s.lite = false;
        s.min_screen_radius = 1.5;
        s.shadow_lod = 80.0;
        match q {
            Quality::Low => (s.shadow_size, s.msaa, s.render_scale) = (1024, 1, 0.75),
            Quality::Medium => (s.shadow_size, s.msaa, s.render_scale) = (2048, 4, 0.9),
            Quality::High | Quality::Ultra => (s.shadow_size, s.msaa, s.render_scale) = (2048, 4, 1.0),
        }
        s
    }

    /// Frame cost per preset at 1080p, the old preset settings against the
    /// new: `cargo test -p arena-client --release -- --ignored gpu_bench
    /// --nocapture`. The figure is throughput (wall ms per frame with frames
    /// in flight, so the GPU stays busy and clocked up; it is the GPU's time
    /// whenever the GPU is the bottleneck); the pass split comes from
    /// timestamp queries on frames drawn one at a time.
    #[test]
    #[ignore]
    fn gpu_bench() {
        let (w, h) = (1920, 1080);
        let (device, queue) = device();
        println!("BENCH adapter: {} ({:?})", device.adapter_info().name, device.adapter_info().backend);
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let wait = |wr: &WorldRenderer| wr.renderer.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        for (map, name) in [(MapId::Sink, "sink"), (MapId::Refinery, "refinery")] {
            let mut worlds = Vec::new();
            for q in [Quality::High, Quality::Medium, Quality::Low] {
                for (label, settings) in [("before", legacy(q)), ("after", q.settings())] {
                    let mut wr = WorldRenderer::new(Renderer::with_settings(device.clone(), queue.clone(), format, w, h, q, settings));
                    let world = scene(&mut wr, map, 20.0, None);
                    worlds.push((q, label, wr, world));
                }
            }
            // Two rounds over every configuration: the first warms the GPU up.
            for round in 0..2 {
                for (q, label, wr, world) in worlds.iter_mut() {
                    for frame in 0..200 {
                        wr.draw(&view, world);
                        if frame % 8 == 7 {
                            wait(wr);
                        }
                    }
                    wait(wr);
                    let frames = 600;
                    let start = std::time::Instant::now();
                    for frame in 0..frames {
                        wr.draw(&view, world);
                        if frame % 8 == 7 {
                            wait(wr);
                        }
                    }
                    wait(wr);
                    let ms = start.elapsed().as_secs_f64() * 1000.0 / frames as f64;
                    // One at a time for the split (a new average every 60).
                    for _ in 0..130 {
                        wr.draw(&view, world);
                        wait(wr);
                    }
                    let g = wr.renderer.gpu_ms;
                    if round == 1 {
                        println!(
                            "BENCH {name:9} {q:?}/{label:6} scale {:.2}: {ms:.2} ms/frame | serial gpu {:.2} ms (shadows {:.2}, scene {:.2}, bloom {:.2}, post {:.2})",
                            wr.renderer.render_scale(),
                            g[0],
                            g[1],
                            g[2],
                            g[3],
                            g[4]
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[ignore]
    fn shots_look() {
        let dir = std::env::var("TANSK_LOOK_DIR").unwrap_or_else(|_| format!("{}/../../docs/shots/look", env!("CARGO_MANIFEST_DIR")));
        std::fs::create_dir_all(&dir).expect("dir");
        let quality = match std::env::var("TANSK_LOOK_QUALITY").as_deref() {
            Ok("low") => Quality::Low,
            Ok("medium") => Quality::Medium,
            Ok("ultra") => Quality::Ultra,
            _ => Quality::High,
        };
        let (w, h) = (1600, 900);
        let (device, queue) = device();
        for (name, map, seconds, view) in VIEWS {
            let mut wr = WorldRenderer::new(Renderer::with_quality(device.clone(), queue.clone(), wgpu::TextureFormat::Rgba8Unorm, w, h, quality));
            let world = scene(&mut wr, map, seconds, view);
            for (tod, t) in [("golden", TimeOfDay::GoldenHour), ("dusk", TimeOfDay::Dusk), ("noon", TimeOfDay::Noon)] {
                wr.renderer.look = Look::preset(t);
                save(&mut wr, &world, w, h, &format!("{dir}/{name}-{tod}.png"));
            }
            if view.is_none() {
                wr.renderer.look = Look::preset(TimeOfDay::GoldenHour);
                wr.renderer.set_storm(0.75);
                save(&mut wr, &world, w, h, &format!("{dir}/{name}-storm.png"));
            }
        }
    }
}
