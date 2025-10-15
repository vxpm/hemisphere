use glam::{Mat4, Vec3A};
use hemisphere::{
    render::Viewport,
    system::gpu::{VertexAttributes, command::attributes::Rgba},
};
use wesl::include_wesl;
use wgpu::util::DeviceExt;
use zerocopy::{Immutable, IntoBytes};

#[derive(Default, Immutable, IntoBytes)]
struct Matrices {
    projection: Mat4,
    position: Mat4,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,

    pipeline: wgpu::RenderPipeline,
    vertex_group_layout: wgpu::BindGroupLayout,
    attributes_group_layout: wgpu::BindGroupLayout,

    viewport: Viewport,
    viewport_tex: wgpu::Texture,

    count: u32,
    matrices: Matrices,
    positions: Vec<Vec3A>,
    diffuse: Vec<Rgba>,
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let viewport_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            dimension: wgpu::TextureDimension::D2,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 1,
        });

        let vertex_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let fragment_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let attribute_buffer_layout_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let attributes_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    attribute_buffer_layout_entry(1),
                    attribute_buffer_layout_entry(2),
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &vertex_group_layout,
                &attributes_group_layout,
                &attributes_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_wesl!("uber").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList, // TODO: don't assume triangle list
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
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
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: Default::default(),
                })],
            }),
            multisample: Default::default(),
            depth_stencil: None,
            multiview: None,
            cache: None,
        });

        Self {
            device,
            queue,

            pipeline,
            vertex_group_layout,
            attributes_group_layout,

            viewport: Viewport {
                width: 1,
                height: 1,
            },
            viewport_tex,

            count: 0,
            matrices: Default::default(),
            positions: Vec::new(),
            diffuse: Vec::new(),
        }
    }

    pub fn viewport_view(&self) -> wgpu::TextureView {
        self.viewport_tex.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            ..Default::default()
        })
    }

    pub fn resize_viewport(&mut self, viewport: Viewport) {
        if viewport == self.viewport {
            return;
        }

        tracing::info!(?viewport, "resizing viewport");
        let viewport_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            dimension: wgpu::TextureDimension::D2,
            size: wgpu::Extent3d {
                width: viewport.width.max(1),
                height: viewport.height.max(1),
                depth_or_array_layers: 1,
            },
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 1,
        });

        self.viewport_tex = viewport_tex;
    }

    pub fn set_position_mat(&mut self, mat: Mat4) {
        self.matrices.position = mat;
    }

    pub fn set_projection_mat(&mut self, mat: Mat4) {
        self.matrices.projection = mat;
    }

    pub fn draw_triangle(&mut self, attributes: Box<VertexAttributes>) {
        self.count += attributes.count as u32;

        if let Some(position) = attributes.position {
            self.positions
                .extend(position.into_iter().map(|v| Vec3A::from(v)));
        }

        if let Some(diffuse) = attributes.diffuse {
            self.diffuse.extend(diffuse.into_iter());
        }

        self.flush();
    }

    pub fn flush(&mut self) {
        let matrices_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: self.matrices.as_bytes(),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let attributes_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: 0u32.as_bytes(),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let positions_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: self.positions.as_bytes(),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let diffuse_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: self.diffuse.as_bytes(),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let vertex_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.vertex_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &matrices_buf,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let attributes_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.attributes_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &attributes_buf,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &positions_buf,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &diffuse_buf,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let viewport = self.viewport_tex.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            ..Default::default()
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &viewport,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, Some(&vertex_group), &[]);
        pass.set_bind_group(1, Some(&attributes_group), &[]);
        pass.set_bind_group(2, Some(&attributes_group), &[]);
        pass.draw(0..self.count, 0..1);

        std::mem::drop(pass);

        let buffer = encoder.finish();
        self.queue.submit([buffer]);

        self.positions.clear();
        self.diffuse.clear();
    }
}
