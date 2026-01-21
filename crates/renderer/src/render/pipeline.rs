mod settings;
mod shader;

use std::borrow::Cow;
use std::collections::hash_map::Entry;

use lazuli::system::gx::CullingMode;
use rustc_hash::FxHashMap;

#[rustfmt::skip]
pub use settings::*;

pub struct Cache {
    group0_layout: wgpu::BindGroupLayout,
    group1_layout: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    cached_pipelines: FxHashMap<Settings, wgpu::RenderPipeline>,
    cached_shaders: FxHashMap<ShaderSettings, wgpu::ShaderModule>,
}

fn split_factor(factor: wgpu::BlendFactor) -> (wgpu::BlendFactor, wgpu::BlendFactor) {
    match factor {
        wgpu::BlendFactor::Src1 => (wgpu::BlendFactor::Src1, wgpu::BlendFactor::Src1Alpha),
        wgpu::BlendFactor::Dst => (wgpu::BlendFactor::Dst, wgpu::BlendFactor::DstAlpha),
        wgpu::BlendFactor::OneMinusSrc1 => (
            wgpu::BlendFactor::OneMinusSrc1,
            wgpu::BlendFactor::OneMinusSrc1Alpha,
        ),
        wgpu::BlendFactor::OneMinusDst => (
            wgpu::BlendFactor::OneMinusDst,
            wgpu::BlendFactor::OneMinusDstAlpha,
        ),
        _ => (factor, factor),
    }
}

fn remove_dst_alpha(factor: wgpu::BlendFactor) -> wgpu::BlendFactor {
    match factor {
        wgpu::BlendFactor::DstAlpha => wgpu::BlendFactor::One,
        wgpu::BlendFactor::OneMinusDstAlpha => wgpu::BlendFactor::Zero,
        _ => factor,
    }
}

impl Cache {
    fn create_pipeline(
        cached_shaders: &mut FxHashMap<ShaderSettings, wgpu::ShaderModule>,
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        settings: &Settings,
        id: u32,
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

        let (color_src, alpha_src) = split_factor(settings.blend.src);
        let (color_dst, alpha_dst) = split_factor(settings.blend.dst);

        let (color_blend, alpha_blend) = if settings.has_alpha {
            let color = wgpu::BlendComponent {
                src_factor: color_src,
                dst_factor: color_dst,
                operation: settings.blend.op,
            };
            let alpha = wgpu::BlendComponent {
                src_factor: alpha_src,
                dst_factor: alpha_dst,
                operation: settings.blend.op,
            };

            (color, alpha)
        } else {
            let color = wgpu::BlendComponent {
                src_factor: remove_dst_alpha(color_src),
                dst_factor: remove_dst_alpha(color_dst),
                operation: settings.blend.op,
            };
            let alpha = wgpu::BlendComponent {
                src_factor: remove_dst_alpha(alpha_src),
                dst_factor: remove_dst_alpha(alpha_dst),
                operation: settings.blend.op,
            };

            (color, alpha)
        };

        let blend = settings.blend.enabled.then_some(wgpu::BlendState {
            color: color_blend,
            alpha: alpha_blend,
        });

        let mut write_mask = wgpu::ColorWrites::empty();
        if settings.blend.color_write {
            write_mask |= wgpu::ColorWrites::COLOR;
        }
        if settings.blend.alpha_write && settings.has_alpha {
            write_mask |= wgpu::ColorWrites::ALPHA;
        }

        let label = format!("shader {}", id);
        let shader = match cached_shaders.entry(settings.shader.clone()) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let shader = shader::compile(&settings.shader);
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(&label),
                    source: wgpu::ShaderSource::Wgsl(Cow::Owned(shader)),
                });

                v.insert(module)
            }
        };

        let cull_mode = match settings.culling {
            CullingMode::None => None,
            CullingMode::Back => Some(wgpu::Face::Back),
            CullingMode::Front => Some(wgpu::Face::Front),
            CullingMode::All => {
                tracing::warn!("culling mode all is not supported - culling back faces only");
                Some(wgpu::Face::Back)
            }
        };

        let label = format!("render pipeline {}", id);
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&label),
            layout: Some(layout),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend,
                    write_mask,
                })],
            }),
            multisample: wgpu::MultisampleState {
                count: 4,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
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

        let buffer = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        let mut current_binding = 0;
        let mut entries = Vec::with_capacity(2 * 8);
        for _ in 0..8 {
            entries.push(tex(current_binding));
            entries.push(sampler(current_binding + 1));
            current_binding += 2;
        }
        entries.push(buffer(current_binding));

        let group1_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &entries,
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&group0_layout, &group1_layout],
            push_constant_ranges: &[],
        });

        Self {
            group0_layout,
            group1_layout,
            layout,
            cached_pipelines: Default::default(),
            cached_shaders: Default::default(),
        }
    }

    pub fn primitives_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.group0_layout
    }

    pub fn textures_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.group1_layout
    }

    pub fn get(&mut self, device: &wgpu::Device, settings: &Settings) -> &wgpu::RenderPipeline {
        let len = self.cached_pipelines.len() as u32;
        match self.cached_pipelines.entry(settings.clone()) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(Self::create_pipeline(
                &mut self.cached_shaders,
                device,
                &self.layout,
                settings,
                len,
            )),
        }
    }
}
