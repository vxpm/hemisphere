#![feature(iter_array_chunks)]
#![feature(btree_cursors)]

mod blit;
mod render;

use crate::{blit::Blitter, render::Renderer};
use flume::{Receiver, Sender};
use hemisphere::{
    render::{Action, Renderer as RendererInterface},
    system::gpu::Topology,
};
use std::sync::{Arc, Mutex};

struct Shared {
    blitter: Blitter,
    frontbuffer: wgpu::TextureView,
}

struct Inner {
    _device: wgpu::Device,
    _queue: wgpu::Queue,
    shared: Arc<Mutex<Shared>>,
    renderer: Renderer,
}

impl Inner {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> (Self, Arc<Mutex<Shared>>) {
        let blitter = Blitter::new(device.clone(), format);
        let renderer = Renderer::new(device.clone(), queue.clone());

        let shared = Shared {
            blitter,
            frontbuffer: renderer.frontbuffer(),
        };
        let shared = Arc::new(Mutex::new(shared));

        (
            Self {
                _device: device,
                _queue: queue,

                shared: shared.clone(),
                renderer,
            },
            shared,
        )
    }

    fn exec(&mut self, action: Action) {
        match action {
            Action::SetViewport(viewport) => {
                self.renderer.resize_viewport(viewport);
                let mut lock = self.shared.lock().unwrap();
                lock.frontbuffer = self.renderer.frontbuffer().clone();
            }
            Action::SetClearColor(color) => self.renderer.set_clear_color(color),
            Action::SetBlendMode(mode) => self.renderer.set_blend_mode(mode),
            Action::SetDepthMode(mode) => self.renderer.set_depth_mode(mode),
            Action::SetProjectionMatrix(mat) => self.renderer.set_projection_mat(mat),
            Action::SetTevStages(stages) => self.renderer.set_tev_stages(stages),
            Action::SetTexGens(texgens) => self.renderer.set_texgens(texgens),
            Action::LoadTexture {
                id,
                width,
                height,
                data,
            } => self.renderer.load_texture(
                id,
                width,
                height,
                zerocopy::transmute_ref!(data.as_slice()),
            ),
            Action::SetTexture { index, id } => self.renderer.set_texture(index, id),
            Action::Draw(topology, attributes) => match topology {
                Topology::QuadList => self.renderer.draw_quad_list(&attributes),
                Topology::TriangleList => self.renderer.draw_triangle_list(&attributes),
                Topology::TriangleStrip => self.renderer.draw_triangle_strip(&attributes),
                Topology::TriangleFan => todo!(),
                Topology::LineList => todo!(),
                Topology::LineStrip => todo!(),
                Topology::PointList => todo!(),
            },
            Action::EfbCopy { clear } => {
                self.renderer.next_pass(clear);
            }
        }
    }
}

#[expect(clippy::needless_pass_by_value, reason = "makes it clearer")]
fn worker(mut renderer: Inner, receiver: Receiver<Action>) {
    while let Ok(action) = receiver.recv() {
        renderer.exec(action);
    }
}

/// A WGPU based renderer implementation.
///
/// This type is reference counted and therefore cheaply clonable.
#[derive(Clone)]
pub struct WgpuRenderer {
    shared: Arc<Mutex<Shared>>,
    sender: Sender<Action>,
}

impl WgpuRenderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let (inner, shared) = Inner::new(device, queue, format);
        let (sender, receiver) = flume::bounded(512);

        std::thread::Builder::new()
            .name("hemisphere wgpu renderer".into())
            .spawn(move || worker(inner, receiver))
            .unwrap();

        Self { shared, sender }
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        let mut shared = self.shared.lock().unwrap();
        let front = shared.frontbuffer.clone();
        shared.blitter.blit(&front, pass);
    }
}

impl RendererInterface for WgpuRenderer {
    fn exec(&mut self, action: Action) {
        self.sender.send(action).expect("rendering thread is alive");
    }
}
