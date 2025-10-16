use std::collections::{HashMap, hash_map::Entry};

use glam::{Mat4, Vec2, Vec3};
use hemisphere::{
    render::Viewport,
    system::gpu::{VertexAttributes, command::attributes::Rgba},
};
use ordered_float::OrderedFloat;
use wesl::include_wesl;
use wgpu::util::DeviceExt;
use zerocopy::{Immutable, IntoBytes};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HashableMat4([OrderedFloat<f32>; 16]);

impl From<Mat4> for HashableMat4 {
    fn from(value: Mat4) -> Self {
        Self(unsafe { std::mem::transmute(value.to_cols_array()) })
    }
}

#[derive(Debug, Clone, Immutable, IntoBytes)]
#[repr(C)]
struct Vertex {
    config_idx: u32,
    projection_idx: u32,

    _pad0: u32,
    _pad1: u32,

    position: Vec3,
    position_mat_idx: u32,

    normal: Vec3,
    normal_mat_idx: u32,

    diffuse: Rgba,
    specular: Rgba,

    tex_coord: [Vec2; 8],
    tex_coord_mat_idx: [u32; 8],
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,

    pipeline: wgpu::RenderPipeline,
    group_layout: wgpu::BindGroupLayout,
    clear_color: wgpu::Color,

    viewport: Viewport,
    viewport_tex: wgpu::Texture,

    current_projection_mat: Mat4,
    current_projection_mat_idx: u32,
    vertices: Vec<Vertex>,
    matrices: Vec<Mat4>,
    matrices_idx: HashMap<HashableMat4, u32>,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&group_layout],
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
            group_layout,
            clear_color: wgpu::Color::BLACK,

            viewport: Viewport {
                width: 1,
                height: 1,
            },
            viewport_tex,

            current_projection_mat: Default::default(),
            current_projection_mat_idx: 0,
            vertices: Vec::new(),
            matrices: Vec::new(),
            matrices_idx: Default::default(),
        }
    }

    pub fn insert_matrix(&mut self, mat: Mat4) -> u32 {
        match self.matrices_idx.entry(mat.clone().into()) {
            Entry::Occupied(o) => *o.get(),
            Entry::Vacant(v) => {
                let idx = self.matrices.len() as u32;
                self.matrices.push(mat);

                *v.insert_entry(idx).get()
            }
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
            label: Some("viewport texture"),
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

        self.viewport = viewport;
        self.viewport_tex = viewport_tex;
    }

    pub fn set_clear_color(&mut self, rgba: Rgba) {
        self.clear_color = wgpu::Color {
            r: rgba.r as f64,
            g: rgba.g as f64,
            b: rgba.b as f64,
            // TODO: deal with alpha properly
            a: 1.0,
        };
    }

    pub fn set_projection_mat(&mut self, mat: Mat4) {
        let id = self.insert_matrix(mat);
        self.current_projection_mat = mat;
        self.current_projection_mat_idx = id;
    }

    pub fn draw_triangle(&mut self, vertices: Vec<VertexAttributes>) {
        for vertex in vertices {
            let position_mat_idx = self.insert_matrix(vertex.position_matrix);
            let normal_mat_idx = self.insert_matrix(Mat4::from_mat3(vertex.normal_matrix));
            let tex_coord_mat_idx = vertex.tex_coord_matrix.map(|mat| self.insert_matrix(mat));

            let vertex = Vertex {
                config_idx: 0,
                projection_idx: self.current_projection_mat_idx,

                _pad0: 0,
                _pad1: 0,

                position: vertex.position,
                position_mat_idx,

                normal: vertex.normal,
                normal_mat_idx,

                diffuse: vertex.diffuse,
                specular: vertex.specular,

                tex_coord: vertex.tex_coord,
                tex_coord_mat_idx,
            };

            self.vertices.push(vertex);
        }
    }

    fn reset(&mut self) {
        self.vertices.clear();
        self.matrices.clear();
        self.matrices_idx.clear();

        self.set_projection_mat(self.current_projection_mat);
    }

    pub fn flush(&mut self) {
        if self.vertices.is_empty() {
            return;
        }

        for vertex in &self.vertices {
            assert!(vertex.projection_idx < self.matrices.len() as u32);
            assert!(vertex.position_mat_idx < self.matrices.len() as u32);
        }

        let configs_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hemisphere configs buffer"),
                contents: &[0, 0, 0, 0],
                usage: wgpu::BufferUsages::STORAGE,
            });

        let matrices_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hemisphere matrices buffer"),
                contents: self.matrices.as_slice().as_bytes(),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let vertices_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hemisphere vertices buffer"),
                contents: self.vertices.as_slice().as_bytes(),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &configs_buf,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &matrices_buf,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &vertices_buf,
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
            label: Some("hemisphere render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &viewport,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, Some(&group), &[]);
        pass.draw(0..self.vertices.len() as u32, 0..1);

        std::mem::drop(pass);

        let buffer = encoder.finish();
        let idx = self.queue.submit([buffer]);

        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(idx),
                timeout: None,
            })
            .unwrap();

        self.reset();
    }
}
