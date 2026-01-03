mod buffers;
mod data;
mod framebuffer;
mod pipeline;
mod textures;

use crate::{
    render::{
        buffers::Buffers,
        framebuffer::Framebuffer,
        pipeline::{Pipeline, TexGenStageSettings},
        textures::Textures,
    },
    util::blit::{ColorBlitter, DepthBlitter},
};
use glam::Mat4;
use hemisphere::{
    modules::render::{Action, TexEnvConfig, TexGenConfig, Viewport, oneshot},
    system::gx::{
        DEPTH_24_BIT_MAX, Topology, Vertex, VertexStream,
        colors::{Rgba, Rgba8},
        pix::{
            self, BlendMode, CompareMode, ConstantAlpha, DepthMode, DstBlendFactor, SrcBlendFactor,
        },
        tev::AlphaFunction,
        xf::ChannelControl,
    },
};
use seq_macro::seq;
use std::{
    num::NonZero,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use zerocopy::IntoBytes;

pub struct Shared {
    pub xfb: Mutex<wgpu::TextureView>,
    pub rendered_anything: AtomicBool,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    shared: Arc<Shared>,

    current_encoder: wgpu::CommandEncoder,
    current_pass: wgpu::RenderPass<'static>,

    pipeline: Pipeline,
    framebuffer: Framebuffer,
    textures: Textures,
    index_buffers: Buffers,
    storage_buffers: Buffers,
    color_copy_buffer: wgpu::Buffer,
    depth_copy_buffer: wgpu::Buffer,

    color_blitter: ColorBlitter,
    depth_blitter: DepthBlitter,

    viewport: Viewport,
    clear_color: wgpu::Color,
    clear_depth: f32,
    current_config: data::Config,
    current_config_dirty: bool,

    vertices: Vec<data::Vertex>,
    indices: Vec<u32>,
    configs: Vec<data::Config>,

    actions: u64,
}

fn set_channel(channel: &mut data::Channel, control: ChannelControl) {
    channel.material_from_vertex = control.material_from_vertex() as u32;
    channel.ambient_from_vertex = control.ambient_from_vertex() as u32;
    channel.lighting_enabled = control.lighting_enabled() as u32;
    channel.diffuse_attenuation = control.diffuse_attenuation() as u32;
    channel.attenuation = control.attenuation() as u32;
    channel.specular = !control.not_specular() as u32;

    let a = control.lights0to3();
    let b = control.lights4to7();
    channel.light_mask = [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]].map(|b| b as u32);
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> (Self, Arc<Shared>) {
        let framebuffer = Framebuffer::new(&device);
        let pipeline = Pipeline::new(&device);
        let textures = Textures::new(&device);
        let index_buffers = Buffers::new(wgpu::BufferUsages::INDEX);
        let storage_buffers = Buffers::new(wgpu::BufferUsages::STORAGE);

        let external = framebuffer.external().create_view(&Default::default());
        let color = framebuffer.color().create_view(&Default::default());
        let multisampled_color = framebuffer
            .multisampled_color()
            .create_view(&Default::default());
        let depth = framebuffer.depth().create_view(&Default::default());

        let shared = Arc::new(Shared {
            xfb: Mutex::new(external),
            rendered_anything: AtomicBool::new(false),
        });

        let mut encoder = device.create_command_encoder(&Default::default());
        let pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hemisphere render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &multisampled_color,
                    depth_slice: None,
                    resolve_target: Some(&color),
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

        let color_blitter = ColorBlitter::new(&device);
        let depth_blitter = DepthBlitter::new(&device);

        let color_copy_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("color copy buffer"),
            size: 640 * 528 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let depth_copy_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("depth copy buffer"),
            size: 640 * 528 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

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
            color_copy_buffer,
            depth_copy_buffer,

            color_blitter,
            depth_blitter,

            viewport: Default::default(),
            clear_color: wgpu::Color::BLACK,
            clear_depth: 1.0,
            current_config: Default::default(),
            current_config_dirty: true,

            vertices: Vec::new(),
            indices: Vec::new(),
            configs: Vec::new(),

            actions: 0,
        };

        value.reset();
        (value, shared)
    }

    pub fn exec(&mut self, action: Action) {
        match action {
            Action::SetFramebufferFormat(fmt) => self.set_framebuffer_format(fmt),
            Action::SetViewport(viewport) => self.set_viewport(viewport),
            Action::SetClearColor(color) => self.set_clear_color(color),
            Action::SetClearDepth(depth) => self.clear_depth = depth,
            Action::SetBlendMode(mode) => self.set_blend_mode(mode),
            Action::SetDepthMode(mode) => self.set_depth_mode(mode),
            Action::SetAlphaFunction(func) => self.set_alpha_function(func),
            Action::SetConstantAlpha(mode) => self.set_constant_alpha_mode(mode),
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
            Action::Draw(topology, vertices) => match topology {
                Topology::QuadList => self.draw_quad_list(&vertices),
                Topology::TriangleList => self.draw_triangle_list(&vertices),
                Topology::TriangleStrip => self.draw_triangle_strip(&vertices),
                Topology::TriangleFan => self.draw_triangle_fan(&vertices),
                Topology::LineList => tracing::warn!("ignored line list primitive"),
                Topology::LineStrip => tracing::warn!("ignored line strip primitive"),
                Topology::PointList => tracing::warn!("ignored point list primitive"),
            },
            Action::SetAmbient(idx, color) => {
                self.current_config.ambient[idx as usize] = color.into();
                self.current_config_dirty = true;
            }
            Action::SetMaterial(idx, color) => {
                self.current_config.material[idx as usize] = color.into();
                self.current_config_dirty = true;
            }
            Action::SetColorChannel(idx, control) => {
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
                let l = &mut self.current_config.lights[idx as usize];
                l.color = light.color.into();
                l.cos_attenuation = light.cos_attenuation;
                l.dist_attenuation = light.dist_attenuation;
                l.position = light.position;
                l.direction = light.direction;

                self.current_config_dirty = true;
            }
            Action::ColorCopy {
                x,
                y,
                width,
                height,
                half,
                clear,
                response,
            } => {
                // println!("color copy requested: ({x}, {y}) [{width}x{height}] (mip: {half})");
                self.debug(format!(
                    "color copy requested: ({x}, {y}) [{width}x{height}] (mip: {half})"
                ));

                self.next_pass(clear, false);
                let data = self.get_color_data(x, y, width, height, half);
                response.send(data).unwrap();
            }
            Action::DepthCopy {
                x,
                y,
                width,
                height,
                half,
                clear,
                response,
            } => {
                self.debug(format!(
                    "depth copy requested: ({x}, {y}) [{width}x{height}] (mip: {half})"
                ));

                self.next_pass(clear, false);
                let data = self.get_depth_data(x, y, width, height, half);
                response.send(data).unwrap();
            }
            Action::XfbCopy { clear } => {
                self.debug("XFB copy requested");
                self.next_pass(clear, true);
            }
        }

        self.actions += 1;
    }

    fn debug(&mut self, s: impl AsRef<str>) {
        let string = s.as_ref();
        let lines = string.lines();
        for line in lines {
            self.current_pass.insert_debug_marker(line);
        }
    }

    fn insert_vertex(&mut self, vertex: &Vertex, matrices: &[(u16, Mat4)]) -> u32 {
        let get_matrix = |idx| {
            matrices
                .iter()
                .find_map(|(i, m)| (*i == idx).then(|| m.clone()))
        };

        let vertex = data::Vertex {
            position: vertex.position,
            config_idx: self.configs.len() as u32 - 1,
            normal: vertex.normal,

            _pad0: 0,

            position_mat: get_matrix(vertex.pos_norm_matrix).unwrap(),
            normal_mat: get_matrix(vertex.pos_norm_matrix + 256).unwrap(),

            chan0: vertex.chan0,
            chan1: vertex.chan1,

            tex_coord: vertex.tex_coords,
            tex_coord_mat: seq! {
                N in 0..8 {
                    [#(get_matrix(vertex.tex_coords_matrix[N]).unwrap(),)*]
                }
            },
        };

        let idx = self.vertices.len();
        self.vertices.push(vertex);

        idx as u32
    }

    pub fn set_framebuffer_format(&mut self, format: pix::BufferFormat) {
        self.debug(format!("set framebuffer format to {format:?}"));
        self.flush("framebuffer format");

        match format {
            pix::BufferFormat::RGB8Z24 | pix::BufferFormat::RGB565Z16 => {
                self.pipeline.settings.has_alpha = false
            }
            pix::BufferFormat::RGBA6Z24 => self.pipeline.settings.has_alpha = true,
            _ => (),
        }
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.debug(format!("set viewport to {viewport:?}"));
        self.current_pass.set_viewport(
            viewport.top_left_x,
            viewport.top_left_y,
            viewport.width,
            viewport.height,
            viewport.near_z.clamp(0.0, 1.0),
            viewport.far_z.clamp(0.0, 1.0),
        );

        self.viewport = viewport;
    }

    pub fn set_clear_color(&mut self, rgba: Rgba) {
        self.debug(format!("set clear color to {rgba:?}"));
        self.clear_color = wgpu::Color {
            r: rgba.r as f64,
            g: rgba.g as f64,
            b: rgba.b as f64,
            a: rgba.a as f64,
        };
    }

    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        let src = match mode.src_factor() {
            SrcBlendFactor::Zero => wgpu::BlendFactor::Zero,
            SrcBlendFactor::One => wgpu::BlendFactor::One,
            SrcBlendFactor::DstColor => wgpu::BlendFactor::Dst,
            SrcBlendFactor::InverseDstColor => wgpu::BlendFactor::OneMinusDst,
            SrcBlendFactor::SrcAlpha => wgpu::BlendFactor::Src1Alpha,
            SrcBlendFactor::InverseSrcAlpha => wgpu::BlendFactor::OneMinusSrc1Alpha,
            SrcBlendFactor::DstAlpha => wgpu::BlendFactor::DstAlpha,
            SrcBlendFactor::InverseDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
        };

        let dst = match mode.dst_factor() {
            DstBlendFactor::Zero => wgpu::BlendFactor::Zero,
            DstBlendFactor::One => wgpu::BlendFactor::One,
            DstBlendFactor::SrcColor => wgpu::BlendFactor::Src1,
            DstBlendFactor::InverseSrcColor => wgpu::BlendFactor::OneMinusSrc1,
            DstBlendFactor::SrcAlpha => wgpu::BlendFactor::Src1Alpha,
            DstBlendFactor::InverseSrcAlpha => wgpu::BlendFactor::OneMinusSrc1Alpha,
            DstBlendFactor::DstAlpha => wgpu::BlendFactor::DstAlpha,
            DstBlendFactor::InverseDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
        };

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

        self.debug(format!("set blend settings to {blend:?}"));
        self.flush("changed blend settings");
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

        self.debug(format!("set depth settings to {depth:?}"));
        self.flush("depth settings changed");
        self.pipeline.settings.depth = depth;
    }

    pub fn set_alpha_function(&mut self, func: AlphaFunction) {
        self.debug(format!("set alpha function to {func:?}"));
        self.flush("alpha function changed");
        self.pipeline.settings.shader.texenv.alpha_func.comparison = func.comparison();
        self.pipeline.settings.shader.texenv.alpha_func.logic = func.logic();
        self.current_config.alpha_refs = func.refs().map(|x| x as u32);
        self.current_config_dirty = true;
    }

    pub fn set_constant_alpha_mode(&mut self, mode: ConstantAlpha) {
        self.debug(format!("set constant alpha mode to {mode:?}"));
        self.current_config.constant_alpha = if mode.enabled() {
            mode.value() as u32
        } else {
            u32::MAX
        };
        self.current_config_dirty = true;
    }

    pub fn set_projection_mat(&mut self, mat: Mat4) {
        self.current_config.projection_mat = mat;
        self.current_config_dirty = true;
    }

    pub fn set_texenv_config(&mut self, config: TexEnvConfig) {
        self.debug("changed texenv");
        self.flush("texenv changed");
        self.pipeline.settings.shader.texenv.stages = config.stages.to_vec();
        self.current_config.consts = config.constants;
        self.current_config_dirty = true;
    }

    pub fn set_texgen_config(&mut self, config: TexGenConfig) {
        self.debug("changed texgen");
        self.flush("texgen changed");
        self.pipeline.settings.shader.texgen.stages = config
            .stages
            .iter()
            .map(|s| TexGenStageSettings {
                base: s.base.clone(),
                normalize: s.normalize,
            })
            .collect();

        for (setting, value) in self
            .current_config
            .post_transform_mat
            .iter_mut()
            .zip(config.stages.iter().map(|s| s.post_matrix))
        {
            *setting = value;
        }

        self.current_config_dirty = true;
    }

    pub fn load_texture(&mut self, id: u32, width: u32, height: u32, data: &[u8]) {
        self.textures
            .update_texture(&self.device, &self.queue, id, width, height, data);
    }

    pub fn set_texture(&mut self, index: usize, id: u32) {
        let in_slot = self.textures.get_texture_slot(index);
        let handle = self
            .textures
            .get_texture(id)
            .expect("texture should exist before being set");

        if in_slot == handle {
            return;
        }

        self.flush("texture slot changed");
        self.textures.set_texture_slot(index, handle);
    }

    fn flush_config(&mut self) {
        if std::mem::take(&mut self.current_config_dirty) {
            self.debug("flushing config");
            self.configs.push(self.current_config.clone());
        }
    }

    pub fn draw_quad_list(&mut self, stream: &VertexStream) {
        let matrices = stream.matrices();
        let vertices = stream.vertices();

        if vertices.is_empty() {
            return;
        }

        self.flush_config();
        self.debug(format!(
            "drawing quad list with {} vertices",
            vertices.len()
        ));

        for vertices in vertices.iter().array_chunks::<4>() {
            let [v0, v1, v2, v3] = vertices.map(|v| self.insert_vertex(v, matrices));
            self.indices.extend_from_slice(&[v0, v1, v2]);
            self.indices.extend_from_slice(&[v0, v2, v3]);
        }
    }

    pub fn draw_triangle_list(&mut self, stream: &VertexStream) {
        let matrices = stream.matrices();
        let vertices = stream.vertices();

        if vertices.is_empty() {
            return;
        }

        self.flush_config();
        self.debug(format!(
            "drawing triangle list with {} vertices",
            vertices.len()
        ));

        for vertices in vertices.iter().array_chunks::<3>() {
            let vertices = vertices.map(|v| self.insert_vertex(v, matrices));
            self.indices.extend_from_slice(&vertices);
        }
    }

    pub fn draw_triangle_strip(&mut self, stream: &VertexStream) {
        let matrices = stream.matrices();
        let vertices = stream.vertices();

        if vertices.is_empty() {
            return;
        }

        self.flush_config();
        self.debug(format!(
            "drawing triangle strip with {} vertices",
            vertices.len()
        ));

        let mut iter = vertices.iter();
        let mut v0 = self.insert_vertex(iter.next().unwrap(), matrices);
        let mut v1 = self.insert_vertex(iter.next().unwrap(), matrices);
        for v2 in iter {
            let v2 = self.insert_vertex(v2, matrices);
            self.indices.extend_from_slice(&[v0, v1, v2]);

            v0 = v1;
            v1 = v2;
        }
    }

    pub fn draw_triangle_fan(&mut self, stream: &VertexStream) {
        let matrices = stream.matrices();
        let vertices = stream.vertices();

        if vertices.is_empty() {
            return;
        }

        self.flush_config();
        self.debug(format!(
            "drawing triangle fan with {} vertices",
            vertices.len()
        ));

        let mut iter = vertices.iter();
        let v0 = self.insert_vertex(iter.next().unwrap(), matrices);
        let mut v1 = self.insert_vertex(iter.next().unwrap(), matrices);
        for v2 in iter {
            let v2 = self.insert_vertex(v2, matrices);
            self.indices.extend_from_slice(&[v0, v1, v2]);

            v1 = v2;
        }
    }

    fn reset(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.configs.clear();
        self.current_config_dirty = true;
    }

    pub fn flush(&mut self, reason: &str) {
        if self.vertices.is_empty() {
            return;
        }

        self.debug(format!("[FLUSH]: {reason}"));
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
    pub fn next_pass(&mut self, clear: bool, copy_to_xfb: bool) {
        self.flush("finishing pass");

        let external = self.framebuffer.external().create_view(&Default::default());
        let color = self.framebuffer.color().create_view(&Default::default());
        let multisampled_color = self
            .framebuffer
            .multisampled_color()
            .create_view(&Default::default());
        let depth = self.framebuffer.depth().create_view(&Default::default());

        let color_op = if clear && self.pipeline.settings.blend.color_write {
            if !self.pipeline.settings.blend.alpha_write {
                tracing::warn!("clearing alpha and color when only color should be cleared!");
            }

            let color = if self.pipeline.settings.has_alpha {
                self.clear_color
            } else {
                wgpu::Color {
                    r: self.clear_color.r,
                    g: self.clear_color.g,
                    b: self.clear_color.b,
                    a: 1.0,
                }
            };

            wgpu::LoadOp::Clear(color)
        } else {
            wgpu::LoadOp::Load
        };

        let depth_op = if clear && self.pipeline.settings.depth.write {
            wgpu::LoadOp::Clear(self.clear_depth)
        } else {
            wgpu::LoadOp::Load
        };

        let mut encoder = self.device.create_command_encoder(&Default::default());
        let pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &multisampled_color,
                    depth_slice: None,
                    resolve_target: Some(&color),
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

        if copy_to_xfb {
            previous_encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfoBase {
                    texture: color.texture(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfoBase {
                    texture: external.texture(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                external.texture().size(),
            );
        }

        let buffer = previous_encoder.finish();
        self.queue.submit([buffer]);
        self.device.poll(wgpu::PollType::Poll).unwrap();

        self.index_buffers.recall();
        self.storage_buffers.recall();

        self.shared.rendered_anything.store(true, Ordering::SeqCst);
    }

    pub fn get_color_data(
        &self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        half: bool,
    ) -> Vec<Rgba8> {
        let color = self.framebuffer.color();

        let divisor = if half { 2 } else { 1 };
        let target_width = width as u32 / divisor;
        let target_height = height as u32 / divisor;

        let row_size = target_width * 4;
        let row_stride = row_size.next_multiple_of(256);

        let copy_target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("color copy texture"),
            dimension: wgpu::TextureDimension::D2,
            size: wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 1,
        });

        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let target_view = copy_target.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        self.color_blitter.blit_to_texture(
            &self.device,
            &color_view,
            wgpu::Origin3d {
                x: x as u32,
                y: y as u32,
                z: 0,
            },
            wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
            &target_view,
            &mut encoder,
        );

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &copy_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::default(),
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.color_copy_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_stride),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
        );

        let (sender, receiver) = oneshot::channel();
        encoder.map_buffer_on_submit(&self.color_copy_buffer, wgpu::MapMode::Read, .., |r| {
            sender.send(r).unwrap()
        });

        let cmd = encoder.finish();
        let submission = self.queue.submit([cmd]);
        self.device
            .poll(wgpu::wgt::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .unwrap();

        let result = receiver.recv().unwrap();
        result.unwrap();

        let mapped = self.color_copy_buffer.get_mapped_range(..);
        let data = &*mapped;

        let mut pixels = Vec::with_capacity(target_width as usize * target_height as usize);
        for row in 0..target_height as usize {
            let row_data = &data[row * row_stride as usize..][..row_size as usize];
            pixels.extend(row_data.chunks_exact(4).map(|c| Rgba8 {
                r: c[0],
                g: c[1],
                b: c[2],
                a: c[3],
            }));
        }

        std::mem::drop(mapped);
        self.color_copy_buffer.unmap();

        pixels
    }

    pub fn get_depth_data(&self, x: u16, y: u16, width: u16, height: u16, half: bool) -> Vec<u32> {
        let depth = self.framebuffer.depth();

        let divisor = if half { 2 } else { 1 };
        let target_width = width as u32 / divisor;
        let target_height = height as u32 / divisor;

        let row_size = target_width * 4;
        let row_stride = row_size.next_multiple_of(256);

        let copy_target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth copy texture"),
            dimension: wgpu::TextureDimension::D2,
            size: wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 1,
        });

        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let target_view = copy_target.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        self.depth_blitter.blit_to_texture(
            &self.device,
            &depth_view,
            wgpu::Origin3d::ZERO,
            wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
            &target_view,
            &mut encoder,
        );

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &copy_target,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: x as u32,
                    y: y as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::default(),
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.depth_copy_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_stride),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: target_width,
                height: target_height,
                depth_or_array_layers: 1,
            },
        );

        let (sender, receiver) = oneshot::channel();
        encoder.map_buffer_on_submit(&self.depth_copy_buffer, wgpu::MapMode::Read, .., |r| {
            sender.send(r).unwrap()
        });

        let cmd = encoder.finish();
        let submission = self.queue.submit([cmd]);
        self.device
            .poll(wgpu::wgt::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .unwrap();

        let result = receiver.recv().unwrap();
        result.unwrap();

        let mapped = self.depth_copy_buffer.get_mapped_range(..);
        let data = &*mapped;

        let mut depth = Vec::with_capacity(target_width as usize * target_height as usize);
        for row in 0..target_height as usize {
            let row_data = &data[row * row_stride as usize..][..row_size as usize];
            depth.extend(row_data.chunks_exact(4).map(|c| {
                let value = f32::from_ne_bytes([c[0], c[1], c[2], c[3]]);

                assert!(value >= 0.0f32);
                assert!(value <= 1.0f32);

                (value * DEPTH_24_BIT_MAX as f32) as u32
            }));
        }

        std::mem::drop(mapped);
        self.depth_copy_buffer.unmap();

        depth
    }
}
