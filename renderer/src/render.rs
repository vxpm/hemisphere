mod data;
mod pipeline;
mod viewport;

use crate::render::{
    pipeline::{Pipeline, PipelineSettings},
    viewport::Framebuffer,
};
use data::*;
use glam::Mat4;
use hemisphere::{
    render::{self, Viewport},
    system::gpu::{
        VertexAttributes,
        command::attributes::Rgba,
        pixel::{CompareMode, DepthMode},
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

    pipeline: Pipeline,
    framebuffer: Framebuffer,

    clear_color: wgpu::Color,
    current_config: Box<Config>,
    current_projection_mat: Mat4,
    current_projection_mat_idx: u32,

    configs: Vec<Config>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    matrices: Vec<Mat4>,
    matrices_idx: FxHashMap<HashableMat4, u32>,
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let framebuffer = Framebuffer::new(&device);
        let pipeline = Pipeline::new(&device);
        let mut value = Self {
            device,
            queue,

            pipeline,
            framebuffer,

            clear_color: wgpu::Color::BLACK,
            current_config: Default::default(),
            current_projection_mat: Default::default(),
            current_projection_mat_idx: 0,

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

    pub fn insert_vertex(&mut self, vertex: Vertex) -> u32 {
        let idx = self.vertices.len();
        self.vertices.push(vertex);

        idx as u32
    }

    fn attributes_to_vertex(&mut self, attributes: &VertexAttributes) -> Vertex {
        let position_mat_idx = self.insert_matrix(attributes.position_matrix);
        let normal_mat_idx = self.insert_matrix(Mat4::from_mat3(attributes.normal_matrix));
        let tex_coord_mat_idx = attributes
            .tex_coords_matrix
            .map(|mat| self.insert_matrix(mat));

        Vertex {
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

    pub fn front_buffer(&self) -> wgpu::TextureView {
        self.framebuffer
            .front()
            .color
            .create_view(&Default::default())
    }

    pub fn swap(&mut self) {
        self.framebuffer.swap();
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

    pub fn set_depth_mode(&mut self, mode: DepthMode) {
        // self.flush(false);

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

        // self.pipeline.switch(
        //     &self.device,
        //     PipelineSettings {
        //         depth_write: mode.update(),
        //         depth_compare: compare,
        //     },
        // );
    }

    pub fn set_projection_mat(&mut self, mat: Mat4) {
        let id = self.insert_matrix(mat);
        self.current_projection_mat = mat;
        self.current_projection_mat_idx = id;
    }

    pub fn set_tev_stages(&mut self, stages: Vec<render::TevStage>) {
        let new = TevConfig::new(stages);
        if self.current_config.tev == new {
            return;
        }

        self.current_config.tev = new;
        self.update_config();
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
            return;
        }

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

        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: self.pipeline.group_layout(),
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

        let framebuffer = self.framebuffer.back();
        let color = framebuffer.color.create_view(&Default::default());
        let depth = framebuffer.depth.create_view(&Default::default());

        let color_load_op = if clear {
            wgpu::LoadOp::Clear(self.clear_color)
        } else {
            wgpu::LoadOp::Load
        };

        let depth_load_op = if clear {
            wgpu::LoadOp::Clear(1.0)
        } else {
            wgpu::LoadOp::Load
        };

        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hemisphere render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: color_load_op,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: depth_load_op,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(self.pipeline.pipeline());
        pass.set_bind_group(0, Some(&group), &[]);
        pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);

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
