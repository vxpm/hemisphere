mod buffers;
mod data;
mod framebuffer;
mod pipeline;
mod textures;

use crate::render::{
    buffers::Buffers, framebuffer::Framebuffer, pipeline::Pipeline, textures::Textures,
};
use glam::Mat4;
use hemisphere::{
    render::{Action, TexEnvConfig, TexGenConfig, Viewport},
    system::gpu::{
        Topology, VertexAttributes,
        colors::Rgba,
        pixel::{BlendFactor, BlendMode, CompareMode, DepthMode},
        transform::ChannelControl,
    },
};
use std::{
    num::NonZero,
    sync::{Arc, Mutex},
};
use zerocopy::IntoBytes;

pub struct Shared {
    pub frontbuffer: wgpu::TextureView,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    shared: Arc<Mutex<Shared>>,

    current_encoder: wgpu::CommandEncoder,
    current_pass: wgpu::RenderPass<'static>,

    pipeline: Pipeline,
    framebuffer: Framebuffer,
    textures: Textures,
    index_buffers: Buffers,
    storage_buffers: Buffers,

    clear_color: wgpu::Color,
    current_projection_mat: Mat4,
    current_config: data::Config,
    current_config_dirty: bool,

    vertices: Vec<data::Vertex>,
    indices: Vec<u32>,
    configs: Vec<data::Config>,
}

fn set_channel(channel: &mut data::Channel, control: ChannelControl) {
    channel.material_from_vertex = control.material_from_vertex() as u32;
    channel.ambient_from_vertex = control.ambient_from_vertex() as u32;
    channel.lighting_enabled = control.lighting_enabled() as u32;
    channel.diffuse_attenuation = control.diffuse_attenuation() as u32;
    channel.attenuation = control.attenuation() as u32;
    channel.spotlight = control.spotlight() as u32;

    let a = control.lights0to3();
    let b = control.lights4to7();
    channel.light_mask = [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]].map(|b| b as u32);
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> (Self, Arc<Mutex<Shared>>) {
        let framebuffer = Framebuffer::new(&device);
        let pipeline = Pipeline::new(&device);
        let textures = Textures::new(&device);
        let index_buffers = Buffers::new(wgpu::BufferUsages::INDEX);
        let storage_buffers = Buffers::new(wgpu::BufferUsages::STORAGE);

        let front = framebuffer.front().create_view(&Default::default());
        let color = framebuffer.color().create_view(&Default::default());
        let depth = framebuffer.depth().create_view(&Default::default());

        let shared = Arc::new(Mutex::new(Shared { frontbuffer: front }));

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
            shared: shared.clone(),

            current_encoder: encoder,
            current_pass: pass,

            pipeline,
            framebuffer,
            textures,
            index_buffers,
            storage_buffers,

            clear_color: wgpu::Color::BLACK,
            current_projection_mat: Default::default(),
            current_config: Default::default(),
            current_config_dirty: true,

            vertices: Vec::new(),
            indices: Vec::new(),
            configs: Vec::new(),
        };

        value.reset();
        (value, shared)
    }

    pub fn exec(&mut self, action: Action) {
        match action {
            Action::SetViewport(viewport) => {
                if self.resize_viewport(viewport) {
                    let mut lock = self.shared.lock().unwrap();
                    lock.frontbuffer = self.frontbuffer().clone();
                }
            }
            Action::SetClearColor(color) => self.set_clear_color(color),
            Action::SetBlendMode(mode) => self.set_blend_mode(mode),
            Action::SetDepthMode(mode) => self.set_depth_mode(mode),
            Action::SetProjectionMatrix(mat) => self.set_projection_mat(mat),
            Action::SetTexEnvConfig(config) => self.set_texenv_config(config),
            Action::SetTexGenConfig(config) => self.set_texgen_config(config),
            Action::LoadTexture {
                id,
                width,
                height,
                data,
            } => self.load_texture(id, width, height, zerocopy::transmute_ref!(data.as_slice())),
            Action::SetTexture { index, id } => self.set_texture(index, id),
            Action::Draw(topology, attributes) => match topology {
                Topology::QuadList => self.draw_quad_list(&attributes),
                Topology::TriangleList => self.draw_triangle_list(&attributes),
                Topology::TriangleStrip => self.draw_triangle_strip(&attributes),
                Topology::TriangleFan => self.draw_triangle_fan(&attributes),
                Topology::LineList => tracing::warn!("ignored line list primitive"),
                Topology::LineStrip => tracing::warn!("ignored line strip primitive"),
                Topology::PointList => tracing::warn!("ignored point list primitive"),
            },
            Action::EfbCopy { clear, to_xfb } => {
                self.next_pass(clear, to_xfb);
            }
            Action::SetAmbient(idx, color) => {
                dbg!(color);
                self.current_config.ambient[idx as usize] = color.into();
                self.current_config_dirty = true;
            }
            Action::SetMaterial(idx, color) => {
                self.current_config.material[idx as usize] = color.into();
                self.current_config_dirty = true;
            }
            Action::SetColorChannel(idx, control) => {
                dbg!(control);
                set_channel(
                    &mut self.current_config.color_channels[idx as usize],
                    control,
                );
                self.current_config_dirty = true;
            }
            Action::SetAlphaChannel(idx, control) => {
                set_channel(
                    &mut self.current_config.alpha_channels[idx as usize],
                    control,
                );
                self.current_config_dirty = true;
            }
            Action::SetLight(idx, light) => {
                dbg!(light);
                let l = &mut self.current_config.lights[idx as usize];
                l.color = light.color.into();
                l.cos_attenuation = light.cos_attenuation;
                l.dist_attenuation = light.dist_attenuation;
                l.position = light.position;
                l.direction = light.direction;

                self.current_config_dirty = true;
            }
        }
    }

    pub fn insert_vertex(&mut self, vertex: data::Vertex) -> u32 {
        let idx = self.vertices.len();
        self.vertices.push(vertex);

        idx as u32
    }

    fn attributes_to_vertex(&mut self, attributes: &VertexAttributes) -> data::Vertex {
        data::Vertex {
            position: attributes.position,
            config_idx: self.configs.len() as u32 - 1,
            normal: attributes.normal,

            _pad0: 0,

            projection_mat: self.current_projection_mat,
            position_mat: attributes.position_matrix,
            normal_mat: attributes.normal_matrix,

            _pad1: 0,
            _pad2: 0,
            _pad3: 0,

            diffuse: attributes.diffuse,
            specular: attributes.specular,

            tex_coord: attributes.tex_coords,
            tex_coord_mat: attributes.tex_coords_matrix,
        }
    }

    pub fn insert_attributes(&mut self, attributes: &VertexAttributes) -> u32 {
        let vertex = self.attributes_to_vertex(attributes);
        self.insert_vertex(vertex)
    }

    pub fn frontbuffer(&self) -> wgpu::TextureView {
        self.framebuffer.front().create_view(&Default::default())
    }

    pub fn resize_viewport(&mut self, viewport: Viewport) -> bool {
        self.framebuffer.resize(&self.device, viewport)
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

        let blend = pipeline::BlendSettings {
            enabled: mode.enable(),
            src,
            dst,
            op,
            color_write: mode.color_mask(),
            alpha_write: mode.alpha_mask(),
        };

        self.flush();
        self.pipeline.settings.blend = blend;
    }

    pub fn set_depth_mode(&mut self, mode: DepthMode) {
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

        let depth = pipeline::DepthSettings {
            enabled: mode.enable(),
            write: mode.update(),
            compare,
        };

        if self.pipeline.settings.depth != depth {
            self.flush();
            self.pipeline.settings.depth = depth;
        }
    }

    pub fn set_projection_mat(&mut self, mat: Mat4) {
        self.current_projection_mat = mat;
    }

    pub fn set_texenv_config(&mut self, config: TexEnvConfig) {
        self.pipeline.settings.texenv = config;
    }

    pub fn set_texgen_config(&mut self, config: TexGenConfig) {
        self.pipeline.settings.texgen = config;
    }

    pub fn load_texture(&mut self, id: u32, width: u32, height: u32, data: &[u8]) {
        self.flush();
        self.textures
            .update_texture(&self.device, &self.queue, id, width, height, data);
    }

    pub fn set_texture(&mut self, index: usize, id: u32) {
        let current = self.textures.get_texture_id(index);
        if current != id {
            self.flush();
            self.textures.set_texture(index, id);
        }
    }

    fn flush_config(&mut self) {
        if std::mem::take(&mut self.current_config_dirty) {
            self.configs.push(self.current_config.clone());
        }
    }

    pub fn draw_quad_list(&mut self, vertices: &[VertexAttributes]) {
        if vertices.is_empty() {
            return;
        }
        self.flush_config();

        for vertices in vertices.iter().array_chunks::<4>() {
            let [v0, v1, v2, v3] = vertices.map(|a| self.insert_attributes(a));
            self.indices.extend_from_slice(&[v0, v1, v2]);
            self.indices.extend_from_slice(&[v0, v2, v3]);
        }
    }

    pub fn draw_triangle_list(&mut self, vertices: &[VertexAttributes]) {
        if vertices.is_empty() {
            return;
        }
        self.flush_config();

        for vertices in vertices.iter().array_chunks::<3>() {
            let vertices = vertices.map(|a| self.insert_attributes(a));
            self.indices.extend_from_slice(&vertices);
        }
    }

    pub fn draw_triangle_strip(&mut self, vertices: &[VertexAttributes]) {
        if vertices.is_empty() {
            return;
        }
        self.flush_config();

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

    pub fn draw_triangle_fan(&mut self, vertices: &[VertexAttributes]) {
        if vertices.is_empty() {
            return;
        }
        self.flush_config();

        let mut iter = vertices.iter();

        let v0 = self.insert_attributes(iter.next().unwrap());
        let mut v1 = self.insert_attributes(iter.next().unwrap());
        for v2 in iter {
            let v2 = self.insert_attributes(v2);
            self.indices.extend_from_slice(&[v0, v1, v2]);

            v1 = v2;
        }
    }

    fn reset(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.configs.clear();

        self.set_projection_mat(self.current_projection_mat);
        self.current_config_dirty = true;
    }

    pub fn flush(&mut self) {
        if self.vertices.is_empty() {
            return;
        }

        self.pipeline.update(&self.device);

        let index_buf =
            self.index_buffers
                .allocate(&self.device, &self.queue, self.indices.as_bytes());
        let vertices_buf =
            self.storage_buffers
                .allocate(&self.device, &self.queue, self.vertices.as_bytes());
        let configs_buf =
            self.storage_buffers
                .allocate(&self.device, &self.queue, self.configs.as_bytes());

        let samplers = self.textures.samplers();
        let textures = self
            .textures
            .textures()
            .clone()
            .map(|tex| tex.create_view(&Default::default()));

        let primitives_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: self.pipeline.primitives_group_layout(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &vertices_buf,
                        offset: 0,
                        size: NonZero::new(self.vertices.as_bytes().len() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &configs_buf,
                        offset: 0,
                        size: NonZero::new(self.configs.as_bytes().len() as u64),
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
        self.current_pass.set_index_buffer(
            index_buf.slice(..self.indices.as_bytes().len() as u64),
            wgpu::IndexFormat::Uint32,
        );
        self.current_pass
            .draw_indexed(0..self.indices.len() as u32, 0, 0..1);

        self.reset();
    }

    // Finishes the current render pass and starts the next one.
    pub fn next_pass(&mut self, clear: bool, to_xfb: bool) {
        self.flush();

        let front = self.framebuffer.front().create_view(&Default::default());
        let color = self.framebuffer.color().create_view(&Default::default());
        let depth = self.framebuffer.depth().create_view(&Default::default());

        let color_op = if clear {
            wgpu::LoadOp::Clear(self.clear_color)
        } else {
            wgpu::LoadOp::Load
        };

        let depth_op = if clear {
            wgpu::LoadOp::Clear(1.0)
        } else {
            wgpu::LoadOp::Load
        };

        let mut encoder = self.device.create_command_encoder(&Default::default());
        let pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hemisphere render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: color_op,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations {
                        load: depth_op,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();

        let mut previous_encoder = std::mem::replace(&mut self.current_encoder, encoder);
        let previous_pass = std::mem::replace(&mut self.current_pass, pass);

        std::mem::drop(previous_pass);

        if to_xfb {
            previous_encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfoBase {
                    texture: color.texture(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfoBase {
                    texture: front.texture(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                front.texture().size(),
            );
        }

        let buffer = previous_encoder.finish();
        self.queue.submit([buffer]);
        self.device.poll(wgpu::PollType::Poll).unwrap();

        self.index_buffers.recall();
        self.storage_buffers.recall();
    }
}
