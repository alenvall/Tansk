//! Bloom: prefilter (exposure, then threshold) into half resolution, a
//! downsample chain, additive tent upsample back up with each wider level
//! scaled down (`scatter`), so the glow stays tight round what is truly
//! bright. The result (level 0, half resolution) is added by the tonemap.
//! The level count is set by the quality preset. Separate textures per
//! level, so no pass reads and writes the same texture (simplest to keep
//! valid on WebGPU).

use crate::renderer::HDR_FORMAT;

pub struct Bloom {
    levels_count: usize,
    prefilter: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    levels: Vec<wgpu::TextureView>,
    /// [prefilter (reads hdr), down 1.., up ..0]
    binds: Vec<wgpu::BindGroup>,
}

impl Bloom {
    /// Pipelines only; `resize` makes the levels.
    pub fn new(device: &wgpu::Device, levels: usize) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/bloom.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
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
            label: Some("bloom pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        let prefilter = pipeline("fs_prefilter", None);
        let down = pipeline("fs_down", None);
        let up = pipeline("fs_up", Some(additive));
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("bloom sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloom uniform"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { levels_count: levels.max(2), prefilter, down, up, layout, sampler, uniform, levels: vec![], binds: vec![] }
    }

    /// The half-resolution result, for the tonemap.
    pub fn output(&self) -> &wgpu::TextureView {
        &self.levels[0]
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    pub fn resize(&mut self, device: &wgpu::Device, hdr: &wgpu::TextureView, width: u32, height: u32) {
        let count = self.levels_count;
        self.levels = (0..count)
            .map(|i| {
                let w = (width >> (i + 1)).max(1);
                let h = (height >> (i + 1)).max(1);
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("bloom level"),
                        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: HDR_FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&wgpu::TextureViewDescriptor::default())
            })
            .collect();
        let bind = |view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloom bind"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            })
        };
        // binds[0] reads hdr; binds[i] (1..LEVELS) reads level i-1.
        let mut binds = vec![bind(hdr)];
        for i in 1..count {
            binds.push(bind(&self.levels[i - 1]));
        }
        // And one more reading the last level, for the first upsample:
        // up-pass into level i reads level i+1, which is binds[i + 2].
        binds.push(bind(&self.levels[count - 1]));
        self.binds = binds;
    }

    /// `threshold` and `knee` apply after `exposure`; `scatter` scales each
    /// wider level as it is added back.
    pub fn record(&self, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, threshold: f32, knee: f32, exposure: f32, scatter: f32) {
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&[threshold, knee, 1.0, exposure, scatter, 0.0, 0.0, 0.0]));
        let pass = |encoder: &mut wgpu::CommandEncoder,
                    target: &wgpu::TextureView,
                    pipeline: &wgpu::RenderPipeline,
                    bind: &wgpu::BindGroup,
                    clear: bool| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if clear { wgpu::LoadOp::Clear(wgpu::Color::BLACK) } else { wgpu::LoadOp::Load },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        };
        pass(encoder, &self.levels[0], &self.prefilter, &self.binds[0], true);
        let count = self.levels.len();
        for i in 1..count {
            pass(encoder, &self.levels[i], &self.down, &self.binds[i], true);
        }
        // Up: level i += scatter * tent(level i+1), from the bottom of the chain.
        for i in (0..count - 1).rev() {
            pass(encoder, &self.levels[i], &self.up, &self.binds[i + 2], false);
        }
    }
}
