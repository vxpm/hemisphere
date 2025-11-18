mod compiler;

use hemisphere::render::{TexEnvConfig, TexGenConfig};
use std::{
    borrow::Cow,
    collections::{HashMap, hash_map::Entry},
};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BlendSettings {
    pub enabled: bool,
    pub src: wgpu::BlendFactor,
    pub dst: wgpu::BlendFactor,
    pub op: wgpu::BlendOperation,

    pub color_write: bool,
    pub alpha_write: bool,
}

impl Default for BlendSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            src: wgpu::BlendFactor::Src,
            dst: wgpu::BlendFactor::Dst,
            op: wgpu::BlendOperation::Add,

            color_write: true,
            alpha_write: true,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DepthSettings {
    pub enabled: bool,
    pub compare: wgpu::CompareFunction,
    pub write: bool,
}

impl Default for DepthSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            compare: wgpu::CompareFunction::Less,
            write: true,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PipelineSettings {
    pub blend: BlendSettings,
    pub depth: DepthSettings,
    pub texenv: TexEnvConfig,
    pub texgen: TexGenConfig,
}

impl Default for PipelineSettings {
    fn default() -> Self {
        Self {
            blend: Default::default(),
            depth: Default::default(),
            texenv: Default::default(),
            texgen: Default::default(),
        }
    }
}

pub struct Pipeline {
    pub settings: PipelineSettings,
    group0_layout: wgpu::BindGroupLayout,
    group1_layout: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    cached: HashMap<PipelineSettings, wgpu::RenderPipeline>,
    pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
    fn create_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        settings: &PipelineSettings,
    ) -> wgpu::RenderPipeline {
        let depth_stencil = if settings.depth.enabled {
            wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: settings.depth.write,
                depth_compare: settings.depth.compare,
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

        let blend_component = wgpu::BlendComponent {
            src_factor: settings.blend.src,
            dst_factor: settings.blend.dst,
            operation: settings.blend.op,
        };

        let blend = settings.blend.enabled.then_some(wgpu::BlendState {
            color: blend_component,
            alpha: blend_component,
        });

        let mut write_mask = wgpu::ColorWrites::empty();
        if settings.blend.color_write {
            write_mask |= wgpu::ColorWrites::COLOR;
        }
        if settings.blend.alpha_write {
            write_mask |= wgpu::ColorWrites::ALPHA;
        }

        let shader = compiler::compile(&settings.texenv, &settings.texgen);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Owned(shader)),
        });

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
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend,
                    write_mask,
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
            entries: &[
                // vertices
                storage_buffer(0),
                // configs
                storage_buffer(1),
            ],
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

        let settings = PipelineSettings::default();
        let pipeline = Self::create_pipeline(device, &layout, &settings);
        let mut cached = HashMap::new();
        cached.insert(settings.clone(), pipeline.clone());

        Self {
            settings,
            group0_layout,
            group1_layout,
            layout,
            cached,
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

    pub fn update(&mut self, device: &wgpu::Device) {
        self.pipeline = match self.cached.entry(self.settings.clone()) {
            Entry::Occupied(o) => o.get().clone(),
            Entry::Vacant(v) => v
                .insert(Self::create_pipeline(device, &self.layout, &self.settings))
                .clone(),
        };
    }
}
