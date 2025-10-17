use wesl::include_wesl;

pub struct PipelineSettings {
    pub depth_write: bool,
    pub depth_compare: wgpu::CompareFunction,
}

pub struct Pipeline {
    group_layout: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    module: wgpu::ShaderModule,
    pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
    fn create_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        module: &wgpu::ShaderModule,
        settings: PipelineSettings,
    ) -> wgpu::RenderPipeline {
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
                module: module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: Default::default(),
                })],
            }),
            multisample: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: settings.depth_write,
                depth_compare: settings.depth_compare,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multiview: None,
            cache: None,
        })
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let group_layout_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                group_layout_entry(0),
                group_layout_entry(1),
                group_layout_entry(2),
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&group_layout],
            push_constant_ranges: &[],
        });

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_wesl!("uber").into()),
        });

        let pipeline = Self::create_pipeline(
            &device,
            &layout,
            &module,
            PipelineSettings {
                depth_write: true,
                depth_compare: wgpu::CompareFunction::Less,
            },
        );

        Self {
            group_layout,
            layout,
            module,
            pipeline,
        }
    }

    pub fn group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.group_layout
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn switch(&mut self, device: &wgpu::Device, settings: PipelineSettings) {
        self.pipeline = Self::create_pipeline(device, &self.layout, &self.module, settings);
    }
}
