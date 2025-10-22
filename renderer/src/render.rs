mod data;
mod pipeline;
mod textures;
mod viewport;

use crate::render::{pipeline::Pipeline, textures::Textures, viewport::Framebuffer};
use glam::Mat4;
use hemisphere::{
    render::{self, Viewport},
    system::gpu::{
        VertexAttributes,
        command::attributes::Rgba,
        pixel::{BlendFactor, BlendMode, CompareMode, DepthMode},
        transform::TexGen,
    },
};
use ordered_float::OrderedFloat;
use rustc_hash::FxHashMap;
use std::collections::hash_map::Entry;
use wgpu::util::DeviceExt;
use zerocopy::IntoBytes;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HashableMat4([OrderedFloat<f32>; 16]);

impl From<Mat4> for HashableMat4 {
    fn from(value: Mat4) -> Self {
        // SAFETY: this is safe because OrderedFloat is repr(transparent)
        Self(unsafe {
            std::mem::transmute::<[f32; 16], [OrderedFloat<f32>; 16]>(value.to_cols_array())
        })
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,

    current_encoder: wgpu::CommandEncoder,
    current_pass: wgpu::RenderPass<'static>,

    pipeline: Pipeline,
    framebuffer: Framebuffer,
    textures: Textures,

    queued_clear: bool,
    clear_color: wgpu::Color,
    current_config: Box<data::Config>,
    current_projection_mat: Mat4,
    current_projection_mat_idx: u32,

    configs: Vec<data::Config>,
    vertices: Vec<data::Vertex>,
    indices: Vec<u32>,
    matrices: Vec<Mat4>,
    matrices_idx: FxHashMap<HashableMat4, u32>,
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let framebuffer = Framebuffer::new(&device);
        let pipeline = Pipeline::new(&device);
        let textures = Textures::new(&device);

        let color = framebuffer.color().create_view(&Default::default());
        let depth = framebuffer.depth().create_view(&Default::default());

        let mut encoder = device.create_command_encoder(&Default::default());
        let pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hemisphere render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();

        let mut value = Self {
            device,
            queue,

            current_encoder: encoder,
            current_pass: pass,

            pipeline,
            framebuffer,
            textures,

            clear_color: wgpu::Color::BLACK,
            current_config: Default::default(),
            current_projection_mat: Default::default(),
            current_projection_mat_idx: 0,

            queued_clear: false,
            configs: Vec::new(),
            vertices: Vec::new(),
            indices: Vec::new(),
            matrices: Vec::new(),
            matrices_idx: Default::default(),
        };

        value.reset();
        value
    }

    pub fn insert_matrix(&mut self, mat: Mat4) -> u32 {
        match self.matrices_idx.entry(mat.into()) {
            Entry::Occupied(o) => *o.get(),
            Entry::Vacant(v) => {
                let idx = self.matrices.len() as u32;
                self.matrices.push(mat);

                *v.insert_entry(idx).get()
            }
        }
    }

    pub fn insert_vertex(&mut self, vertex: data::Vertex) -> u32 {
        let idx = self.vertices.len();
        self.vertices.push(vertex);

        idx as u32
    }

    fn attributes_to_vertex(&mut self, attributes: &VertexAttributes) -> data::Vertex {
        let position_mat_idx = self.insert_matrix(attributes.position_matrix);
        let normal_mat_idx = self.insert_matrix(Mat4::from_mat3(attributes.normal_matrix));
        let tex_coord_mat_idx = attributes
            .tex_coords_matrix
            .map(|mat| self.insert_matrix(mat));

        data::Vertex {
            config_idx: self.configs.len() as u32 - 1,
            projection_idx: self.current_projection_mat_idx,

            _pad0: 0,
            _pad1: 0,

            position: attributes.position,
            position_mat_idx,

            normal: attributes.normal,
            normal_mat_idx,

            diffuse: attributes.diffuse,
            specular: attributes.specular,

            tex_coord: attributes.tex_coords,
            tex_coord_mat_idx,
        }
    }

    pub fn insert_attributes(&mut self, attributes: &VertexAttributes) -> u32 {
        let vertex = self.attributes_to_vertex(attributes);
        self.insert_vertex(vertex)
    }

    pub fn update_config(&mut self) {
        self.configs.push((*self.current_config).clone());
    }

    pub fn framebuffer(&self) -> wgpu::TextureView {
        self.framebuffer.color().create_view(&Default::default())
    }

    pub fn swap(&mut self) {
        self.flush(false);
    }

    pub fn resize_viewport(&mut self, viewport: Viewport) {
        self.framebuffer.resize(&self.device, viewport);
    }

    pub fn set_clear_color(&mut self, rgba: Rgba) {
        self.clear_color = wgpu::Color {
            r: rgba.r as f64,
            g: rgba.g as f64,
            b: rgba.b as f64,
            a: rgba.a as f64,
        };
    }

    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        self.flush(false);

        let factor = |factor: BlendFactor| match factor {
            BlendFactor::Zero => wgpu::BlendFactor::Zero,
            BlendFactor::One => wgpu::BlendFactor::One,
            BlendFactor::SrcColor => wgpu::BlendFactor::Src,
            BlendFactor::InverseSrcColor => wgpu::BlendFactor::OneMinusSrc,
            BlendFactor::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
            BlendFactor::InverseSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
            BlendFactor::DstAlpha => wgpu::BlendFactor::DstAlpha,
            BlendFactor::InverseDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
        };

        let src = factor(mode.src_factor());
        let dst = factor(mode.dst_factor());
        let op = if mode.blend_subtract() {
            wgpu::BlendOperation::Subtract
        } else {
            wgpu::BlendOperation::Add
        };

        self.pipeline.settings.color_write = mode.color_mask();
        self.pipeline.settings.alpha_write = mode.alpha_mask();
        self.pipeline.settings.blend_enabled = mode.enable();
        self.pipeline.settings.blend_src = src;
        self.pipeline.settings.blend_dst = dst;
        self.pipeline.settings.blend_op = op;
        self.pipeline.update(&self.device);
    }

    pub fn set_depth_mode(&mut self, mode: DepthMode) {
        self.flush(false);

        let compare = match mode.compare() {
            CompareMode::Never => wgpu::CompareFunction::Never,
            CompareMode::Less => wgpu::CompareFunction::Less,
            CompareMode::Equal => wgpu::CompareFunction::Equal,
            CompareMode::LessOrEqual => wgpu::CompareFunction::LessEqual,
            CompareMode::Greater => wgpu::CompareFunction::Greater,
            CompareMode::NotEqual => wgpu::CompareFunction::NotEqual,
            CompareMode::GreaterOrEqual => wgpu::CompareFunction::GreaterEqual,
            CompareMode::Always => wgpu::CompareFunction::Always,
        };

        self.pipeline.settings.depth_enabled = mode.enable();
        self.pipeline.settings.depth_write = mode.update();
        self.pipeline.settings.depth_compare = compare;
        self.pipeline.update(&self.device);
    }

    pub fn set_projection_mat(&mut self, mat: Mat4) {
        let id = self.insert_matrix(mat);
        self.current_projection_mat = mat;
        self.current_projection_mat_idx = id;
    }

    pub fn set_tev_stages(&mut self, stages: Vec<render::TevStage>) {
        let new = data::TevConfig::new(stages.clone());
        if self.current_config.tev == new {
            return;
        }

        // println!("stages: {stages:#?}");

        self.current_config.tev = new;
        self.update_config();
    }

    pub fn set_texgens(&mut self, texgens: Vec<TexGen>) {
        let new = data::TexGenConfig::new(texgens);
        if self.current_config.texgen == new {
            return;
        }

        self.current_config.texgen = new;
        self.update_config();
    }

    pub fn load_texture(&mut self, id: u32, width: u32, height: u32, data: &[u8]) {
        self.flush(false);
        self.textures
            .update_texture(&self.device, &self.queue, id, width, height, data);
    }

    pub fn set_texture(&mut self, index: usize, id: u32) {
        let current = self.textures.get_texture_id(index);
        if current != id {
            self.flush(false);
            self.textures.set_texture(index, id);
        }
    }

    pub fn draw_quad_list(&mut self, vertices: &[VertexAttributes]) {
        for vertices in vertices.iter().array_chunks::<4>() {
            let [v0, v1, v2, v3] = vertices.map(|a| self.insert_attributes(a));
            self.indices.extend_from_slice(&[v0, v1, v2]);
            self.indices.extend_from_slice(&[v0, v2, v3]);
        }
    }

    pub fn draw_triangle_list(&mut self, vertices: &[VertexAttributes]) {
        for vertices in vertices.iter().array_chunks::<3>() {
            let vertices = vertices.map(|a| self.insert_attributes(a));
            self.indices.extend_from_slice(&vertices);
        }
    }

    pub fn draw_triangle_strip(&mut self, vertices: &[VertexAttributes]) {
        let mut iter = vertices.iter();

        let mut v0 = self.insert_attributes(iter.next().unwrap());
        let mut v1 = self.insert_attributes(iter.next().unwrap());
        for v2 in iter {
            let v2 = self.insert_attributes(v2);
            self.indices.extend_from_slice(&[v0, v1, v2]);

            v0 = v1;
            v1 = v2;
        }
    }

    fn reset(&mut self) {
        self.configs.clear();
        self.vertices.clear();
        self.indices.clear();
        self.matrices.clear();
        self.matrices_idx.clear();

        self.update_config();
        self.set_projection_mat(self.current_projection_mat);
    }

    pub fn flush(&mut self, clear: bool) {
        if self.vertices.is_empty() {
            self.queued_clear |= clear;
            return;
        }

        let clear = clear || self.queued_clear;
        self.queued_clear = false;

        let index_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hemisphere index buffer"),
                contents: self.indices.as_bytes(),
                usage: wgpu::BufferUsages::INDEX,
            });

        let configs_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hemisphere configs buffer"),
                contents: self.configs.as_bytes(),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let matrices_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hemisphere matrices buffer"),
                contents: self.matrices.as_bytes(),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let vertices_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hemisphere vertices buffer"),
                contents: self.vertices.as_bytes(),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let textures = self
            .textures
            .textures()
            .clone()
            .map(|tex| tex.create_view(&Default::default()));
        let samplers = self.textures.samplers();

        let primitives_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: self.pipeline.primitives_group_layout(),
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

        let textures_group_entries: [wgpu::BindGroupEntry; 16] = std::array::from_fn(|binding| {
            let tex = binding / 2;
            if binding % 2 == 0 {
                wgpu::BindGroupEntry {
                    binding: binding as u32,
                    resource: wgpu::BindingResource::TextureView(&textures[tex]),
                }
            } else {
                wgpu::BindGroupEntry {
                    binding: binding as u32,
                    resource: wgpu::BindingResource::Sampler(&samplers[tex]),
                }
            }
        });
        let textures_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: self.pipeline.textures_group_layout(),
            entries: &textures_group_entries,
        });

        self.current_pass.set_pipeline(self.pipeline.pipeline());
        self.current_pass
            .set_bind_group(0, Some(&primitives_group), &[]);
        self.current_pass
            .set_bind_group(1, Some(&textures_group), &[]);
        self.current_pass
            .set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
        self.current_pass
            .draw_indexed(0..self.indices.len() as u32, 0, 0..1);

        self.reset();

        if clear {
            let color = self.framebuffer.color().create_view(&Default::default());
            let depth = self.framebuffer.depth().create_view(&Default::default());

            let mut encoder = self.device.create_command_encoder(&Default::default());
            let pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("hemisphere render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &color,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(self.clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();

            let previous_encoder = std::mem::replace(&mut self.current_encoder, encoder);
            let previous_pass = std::mem::replace(&mut self.current_pass, pass);

            std::mem::drop(previous_pass);

            let buffer = previous_encoder.finish();
            self.queue.submit([buffer]);
        }
    }
}
