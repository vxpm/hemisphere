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

struct Inner {
    _device: wgpu::Device,
    _queue: wgpu::Queue,
    blitter: Blitter,
    renderer: Renderer,
}

impl Inner {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let blitter = Blitter::new(device.clone(), format);
        let renderer = Renderer::new(device.clone(), queue.clone());

        Self {
            _device: device,
            _queue: queue,

            blitter,
            renderer,
        }
    }

    fn exec(&mut self, action: Action) {
        match action {
            Action::SetViewport(viewport) => self.renderer.resize_viewport(viewport),
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
fn worker(inner: Arc<Mutex<Inner>>, receiver: Receiver<Action>) {
    loop {
        std::thread::yield_now();
        let Ok(action) = receiver.recv() else {
            // sender has been dropped
            return;
        };

        {
            let mut renderer = inner.lock().unwrap();
            renderer.exec(action);

            let mut count = 0;
            while let Ok(action) = receiver.try_recv()
                && count < 256
            {
                renderer.exec(action);
                count += 1;
            }
        }
    }
}

/// A WGPU based renderer implementation.
///
/// This type is reference counted and therefore cheaply clonable.
#[derive(Clone)]
pub struct WgpuRenderer {
    inner: Arc<Mutex<Inner>>,
    sender: Sender<Action>,
}

impl WgpuRenderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let inner = Arc::new(Mutex::new(Inner::new(device, queue, format)));
        let (sender, receiver) = flume::bounded(512);

        std::thread::Builder::new()
            .name("hemisphere wgpu renderer".into())
            .spawn({
                let inner = inner.clone();
                move || worker(inner, receiver)
            })
            .unwrap();

        Self { inner, sender }
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        let mut guard = self.inner.lock().unwrap();
        let inner = &mut *guard;

        inner.blitter.blit(&inner.renderer.framebuffer(), pass);
    }
}

impl RendererInterface for WgpuRenderer {
    fn exec(&mut self, action: Action) {
        self.sender.send(action).expect("rendering thread is alive");
    }
}
