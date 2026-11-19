//! The wgpu renderer: three-cascade shadow maps, an MSAA HDR forward pass
//! with instanced meshes, effect sprites and rut decals, bloom, tonemap.
//! The same code drives the browser (WebGPU), the native window (Vulkan)
//! and the headless `--shot` path; only the target texture differs.
//!
//! Meshes are uploaded once ([`Renderer::add_mesh`]); every frame the caller
//! hands over a flat list of [`Instance`]s, which are frustum-culled per
//! view (camera and each cascade), sorted by mesh and drawn one instanced
//! call per mesh.

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
}

/// What a preset sets.
#[derive(Clone, Copy, Debug)]
pub struct QualitySettings {
    pub shadow_size: u32,
    pub msaa: u32,
    /// The highest render scale; dynamic scaling stays at or below it.
    pub render_scale: f32,
    /// Effect particle cap (see `Fx::set_max_particles`).
    pub max_particles: usize,
    /// How far blob contact shadows are drawn from the eye.
    pub blob_range: f32,
}

impl Quality {
    pub fn settings(self) -> QualitySettings {
        match self {
            Quality::Low => QualitySettings { shadow_size: 1024, msaa: 1, render_scale: 0.75, max_particles: 1500, blob_range: 60.0 },
            Quality::Medium => QualitySettings { shadow_size: 2048, msaa: 4, render_scale: 0.9, max_particles: 3000, blob_range: 100.0 },
            Quality::High => QualitySettings { shadow_size: 2048, msaa: 4, render_scale: 1.0, max_particles: 5000, blob_range: 140.0 },
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
    pub bloom: f32,
    pub bloom_threshold: f32,
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
                l.sky_color = Vec3::new(0.36, 0.38, 0.62) * 0.4;
                l.ground_color = Vec3::new(0.9, 0.45, 0.25) * 0.45;
                l.haze = Vec3::new(1.0, 0.66, 0.46) * 1.1;
                l.warmth = Vec3::new(1.06, 0.98, 0.9);
            }
            TimeOfDay::Noon => {
                l.sun_elevation = 62.0;
                l.sun_color = Vec3::new(1.0, 0.94, 0.84) * 2.6;
                l.sky_color = Vec3::new(0.40, 0.52, 0.78) * 0.5;
                l.haze = Vec3::new(0.96, 0.9, 0.8) * 1.25;
                l.warmth = Vec3::new(1.0, 1.0, 0.98);
            }
        }
        l.sun_dir = l.sun_direction();
        l
    }
}

impl Default for Look {
    fn default() -> Self {
        // Golden hour: a low warm sun about 20 degrees up.
        let (elev, az) = (20.0f32, 140.0f32);
        let sun_dir = Vec3::new(az.to_radians().cos() * elev.to_radians().cos(), elev.to_radians().sin(), az.to_radians().sin() * elev.to_radians().cos());
        Self {
            sun_elevation: elev,
            sun_azimuth: az,
            sun_dir,
            sun_color: Vec3::new(1.0, 0.70, 0.42) * 2.8,
            sky_color: Vec3::new(0.40, 0.48, 0.70) * 0.42,
            ground_color: Vec3::new(0.95, 0.56, 0.28) * 0.6,
            haze: Vec3::new(1.0, 0.86, 0.68) * 1.25,
            haze_start: 120.0,
            haze_density: 0.0028,
            height_fog: 0.0005,
            height_falloff: 0.18,
            mie_g: 0.85,
            air_glow: 0.12,
            shadow_softness: 1.3,
            splits: [24.0, 75.0, 230.0],
            rim: 1.0,
            spec: 1.0,
            exposure: 1.0,
            vignette: 0.3,
            saturation: 1.12,
            warmth: Vec3::new(1.04, 1.0, 0.93),
            bloom: 0.16,
            bloom_threshold: 0.85,
            contrast: 1.1,
            storm: 0.0,
            curve: 0,
        }
    }
}

/// A mesh uploaded with [`Renderer::add_mesh`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshId(pub u32);

/// One placed mesh. `params` reaches the shader per instance (x/z tread
/// scroll, y hit flash).
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pub mesh: MeshId,
    pub model: Mat4,
    pub params: Vec4,
}

impl Instance {
    pub fn new(mesh: MeshId, model: Mat4) -> Self {
        Self { mesh, model, params: Vec4::ZERO }
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
}

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
}

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
        let settings = quality.settings();
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
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

        let vertex_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3];
        let instance_attributes = wgpu::vertex_attr_array![4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4];
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
                targets: &[Some(wgpu::ColorTargetState { format: output_format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
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
        let mut bloom = Bloom::new(&device);
        let scale = settings.render_scale;
        let (iw, ih) = scaled(width, height, scale);
        let targets = make_targets(&device, &post_layout, &post_uniform, &mut bloom, iw, ih, msaa_count);
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
            quality,
            settings,
            output_format,
            scale,
            frame_avg: 1.0 / 60.0,
            scale_cooldown: 2.0,
            frame_budget: 1.0 / 60.0,
        }
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
    /// 0.6 and the preset's) to hold `frame_budget`.
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
        let max = self.settings.render_scale;
        let mut next = self.scale;
        if self.frame_avg > self.frame_budget * 1.12 {
            next = (self.scale - 0.1).max(0.6);
        } else if self.frame_avg < self.frame_budget * 0.8 {
            next = (self.scale + 0.05).min(max);
        }
        if (next - self.scale).abs() > 1e-3 {
            self.scale = next;
            let (iw, ih) = scaled(self.width, self.height, self.scale);
            self.targets = make_targets(&self.device, &self.post_layout, &self.post_uniform, &mut self.bloom, iw, ih, self.settings.msaa);
            log::info!("render scale {:.2} (frame {:.1} ms)", self.scale, self.frame_avg * 1000.0);
        }
        self.scale_cooldown = 1.0;
    }

    #[allow(dead_code)] // wired to the Settings menu by the game layer
    pub fn render_scale(&self) -> f32 {
        self.scale
    }

    pub fn add_mesh(&mut self, m: &MeshBuilder) -> MeshId {
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for v in &m.vertices {
            lo = lo.min(Vec3::from(v.position));
            hi = hi.max(Vec3::from(v.position));
        }
        let (centre, radius) = if m.vertices.is_empty() { (Vec3::ZERO, 0.0) } else { ((lo + hi) * 0.5, (hi - lo).length() * 0.5) };
        let empty = m.indices.is_empty();
        let verts: &[u8] = if empty { &[0u8; 48] } else { bytemuck::cast_slice(&m.vertices) };
        let idx: &[u8] = if empty { &[0u8; 12] } else { bytemuck::cast_slice(&m.indices) };
        let mesh = GpuMesh {
            vertices: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("mesh vertices"), contents: verts, usage: wgpu::BufferUsages::VERTEX }),
            indices: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("mesh indices"), contents: idx, usage: wgpu::BufferUsages::INDEX }),
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
        id
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
        self.targets = make_targets(&self.device, &self.post_layout, &self.post_uniform, &mut self.bloom, iw, ih, self.settings.msaa);
    }

    /// Fits each cascade round its slice of the view, snapped to texels.
    fn cascades(&self, camera: &Camera, aspect: f32) -> [(Mat4, Mat4, f32); CASCADES] {
        let sun = self.look.sun_direction();
        let fwd = (camera.target - camera.eye).normalize();
        let tan = (camera.fov_y * 0.5).tan();
        let rot = look_at_mat4(Vec3::ZERO, -sun, Vec3::Y);
        let inv_rot = rot.inverse();
        let mut near = 0.1;
        let mut out = [(Mat4::IDENTITY, Mat4::IDENTITY, 0.0); CASCADES];
        for (i, slot) in out.iter_mut().enumerate() {
            let far = self.look.splits[i];
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
        let look = &self.look;
        let aspect = self.width as f32 / self.height as f32;
        let view = look_at_mat4(camera.eye, camera.target, Vec3::Y);
        let projm = proj::perspective(camera.fov_y, aspect, 0.1, 3000.0);
        let view_proj = projm * view;
        let cascades = self.cascades(camera, aspect);
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
        for (i, l) in lights.iter().take(MAX_LIGHTS).enumerate() {
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
        let look = &l;
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            cascades: [cascades[0].0.to_cols_array_2d(), cascades[1].0.to_cols_array_2d(), cascades[2].0.to_cols_array_2d()],
            camera: camera.eye.extend(time).to_array(),
            cam_right: right.extend(0.0).to_array(),
            cam_up: up.extend(0.0).to_array(),
            sun_dir: look.sun_dir.normalize().extend(0.0).to_array(),
            sun_color: look.sun_color.extend(0.0).to_array(),
            sky_color: look.sky_color.extend(0.0).to_array(),
            ground_color: look.ground_color.extend(0.0).to_array(),
            fog: look.haze.extend(look.haze_start).to_array(),
            fog2: [look.haze_density, look.height_fog, look.height_falloff, look.mie_g],
            shadow: [shadow_size, look.shadow_softness, look.air_glow, look.storm],
            splits: [look.splits[0], look.splits[1], look.splits[2], 0.0],
            texels: [texel(cascades[0].2), texel(cascades[1].2), texel(cascades[2].2), 0.0],
            ranges: [2.0 * cascades[0].2 + SHADOW_BACK, 2.0 * cascades[1].2 + SHADOW_BACK, 2.0 * cascades[2].2 + SHADOW_BACK, 0.0],
            misc: [occluders.len().min(MAX_OCCLUDERS) as f32, lights.len().min(MAX_LIGHTS) as f32, look.rim, look.spec],
            occluders: occ,
            light_pos,
            light_col,
        };
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        for (i, (buf, _)) in self.cascade_binds.iter().enumerate() {
            self.queue.write_buffer(buf, 0, bytemuck::bytes_of(&cascades[i].0.to_cols_array_2d()));
        }
        let post = [
            Vec4::new(look.exposure, look.vignette, if self.encode_srgb { 1.0 } else { 0.0 }, look.curve as f32),
            look.warmth.extend(look.saturation),
            Vec4::new(look.bloom, look.contrast, 1.0 / self.width as f32, 1.0 / self.height as f32),
        ];
        self.queue.write_buffer(&self.post_uniform, 0, bytemuck::cast_slice(&post));

        // Cull per view, sort by mesh, pack all views into one buffer.
        let planes = frustum_planes(view_proj);
        let mut packed: Vec<GpuInstance> = Vec::with_capacity(instances.len() * 2);
        let mut batches: Vec<Vec<(MeshId, u32, u32)>> = Vec::new();
        let mut views: Vec<(Box<dyn Fn(Vec3, f32) -> bool>, bool)> = Vec::new();
        views.push((Box::new(move |c, r| sphere_in_planes(&planes, c, r)), true));
        for c in cascades.iter() {
            let (lv, radius) = (c.1, c.2);
            views.push((
                Box::new(move |p: Vec3, r: f32| {
                    let q = lv.transform_point3(p);
                    q.x.abs() < radius + r && q.y.abs() < radius + r && -q.z > -r && -q.z < 2.0 * radius + SHADOW_BACK + r
                }),
                false,
            ));
        }
        let mut visible: Vec<(MeshId, usize)> = Vec::new();
        for (test, main) in &views {
            visible.clear();
            for (k, inst) in instances.iter().enumerate() {
                let Some(mesh) = self.meshes.get(inst.mesh.0 as usize).and_then(|m| m.as_ref()) else { continue };
                if mesh.count == 0 {
                    continue;
                }
                let c = inst.model.transform_point3(mesh.centre);
                let scale = inst.model.x_axis.truncate().length().max(inst.model.y_axis.truncate().length()).max(inst.model.z_axis.truncate().length());
                let r = mesh.radius * scale;
                if test(c, r) {
                    // Shadows skip tiny things far away from the eye.
                    if !*main && r < 1.0 && c.distance(camera.eye) > 60.0 {
                        continue;
                    }
                    visible.push((inst.mesh, k));
                }
            }
            visible.sort_by_key(|v| v.0);
            let mut list = Vec::new();
            let mut i = 0;
            while i < visible.len() {
                let mesh = visible[i].0;
                let first = packed.len() as u32;
                while i < visible.len() && visible[i].0 == mesh {
                    let inst = &instances[visible[i].1];
                    packed.push(GpuInstance { model: inst.model.to_cols_array_2d(), params: inst.params.to_array() });
                    i += 1;
                }
                list.push((mesh, first, packed.len() as u32 - first));
            }
            batches.push(list);
        }
        self.instances.write(&self.device, &self.queue, bytemuck::cast_slice(&packed));
        self.sprites.write(&self.device, &self.queue, bytemuck::cast_slice(sprites));
        self.ruts.write(&self.device, &self.queue, bytemuck::cast_slice(ruts));
        let range = self.settings.blob_range;
        let near_blobs: Vec<Vec4> = blobs.iter().copied().filter(|b| b.truncate().distance(camera.eye) < range).collect();
        self.blobs.write(&self.device, &self.queue, bytemuck::cast_slice(&near_blobs));
        self.stats = (batches.iter().map(|b| b.len() as u32).sum(), packed.len() as u32);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        for (i, layer) in self.shadow_layers.iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: layer,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
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
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
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
            if !sprites.is_empty() {
                pass.set_pipeline(&self.sprite_pipeline);
                pass.set_vertex_buffer(0, self.sprites.buffer.slice(..));
                pass.draw(0..6, 0..sprites.len() as u32);
            }
        }
        self.bloom.record(&self.queue, &mut encoder, self.look.bloom_threshold, 0.6);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("post pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.post_pipeline);
            pass.set_bind_group(0, &self.targets.post_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
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

fn make_targets(device: &wgpu::Device, post_layout: &wgpu::BindGroupLayout, post_uniform: &wgpu::Buffer, bloom: &mut Bloom, width: u32, height: u32, msaa: u32) -> Targets {
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
    let depth = make("depth", DEPTH_FORMAT, msaa, wgpu::TextureUsages::RENDER_ATTACHMENT);
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
    Targets { msaa_color, depth, hdr, post_bind }
}

/// Requests a device with WebGPU-portable limits.
pub async fn request_device(adapter: &wgpu::Adapter) -> anyhow::Result<(wgpu::Device, wgpu::Queue)> {
    let info = adapter.get_info();
    log::info!("adapter: {} ({:?}, {:?})", info.name, info.backend, info.device_type);
    let limits = wgpu::Limits::default().using_resolution(adapter.limits());
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("tansk device"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::anyhow!("requesting a wgpu device: {e}"))?;
    Ok((device, queue))
}
