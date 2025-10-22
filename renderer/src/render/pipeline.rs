use wesl::include_wesl;

pub struct PipelineSettings {
    pub depth_enabled: bool,
    pub depth_write: bool,
    pub depth_compare: wgpu::CompareFunction,
}

pub struct Pipeline {
    group0_layout: wgpu::BindGroupLayout,
    group1_layout: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    module: wgpu::ShaderModule,
    pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
    fn create_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        module: &wgpu::ShaderModule,
        settings: &PipelineSettings,
    ) -> wgpu::RenderPipeline {
        let depth_stencil = if settings.depth_enabled {
            wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: settings.depth_write,
                depth_compare: settings.depth_compare,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }
        } else {
            wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }
        };

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uber render pipeline"),
            layout: Some(layout),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: Default::default(),
                })],
            }),
            multisample: Default::default(),
            depth_stencil: Some(depth_stencil),
            multiview: None,
            cache: None,
        })
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let storage_buffer = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let group0_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[storage_buffer(0), storage_buffer(1), storage_buffer(2)],
        });

        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let group1_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                tex(0),
                sampler(1),
                tex(2),
                sampler(3),
                tex(4),
                sampler(5),
                tex(6),
                sampler(7),
                tex(8),
                sampler(9),
                tex(10),
                sampler(11),
                tex(12),
                sampler(13),
                tex(14),
                sampler(15),
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&group0_layout, &group1_layout],
            push_constant_ranges: &[],
        });

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_wesl!("uber").into()),
        });

        let pipeline = Self::create_pipeline(
            device,
            &layout,
            &module,
            &PipelineSettings {
                depth_enabled: true,
                depth_write: true,
                depth_compare: wgpu::CompareFunction::Less,
            },
        );

        Self {
            group0_layout,
            group1_layout,
            layout,
            module,
            pipeline,
        }
    }

    pub fn primitives_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.group0_layout
    }

    pub fn textures_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.group1_layout
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn switch(&mut self, device: &wgpu::Device, settings: &PipelineSettings) {
        self.pipeline = Self::create_pipeline(device, &self.layout, &self.module, settings);
    }
}
