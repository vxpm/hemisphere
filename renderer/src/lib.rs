#![feature(iter_array_chunks)]

mod blit;
mod render;

use crate::{blit::Blitter, render::Renderer};
use hemisphere::{
    render::{Action, Renderer as RendererInterface},
    system::gpu::Topology,
};
use std::sync::{
    Arc, Mutex,
    mpsc::{Receiver, Sender, channel},
};

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
            Action::SetProjectionMatrix(mat) => self.renderer.set_projection_mat(mat),
            Action::SetTevStages(stages) => self.renderer.set_tev_stages(stages),
            Action::Draw(topology, attributes) => match topology {
                Topology::QuadList => self.renderer.draw_quad_list(attributes),
                Topology::TriangleList => self.renderer.draw_triangle_list(attributes),
                Topology::TriangleStrip => self.renderer.draw_triangle_strip(attributes),
                Topology::TriangleFan => todo!(),
                Topology::LineList => todo!(),
                Topology::LineStrip => todo!(),
                Topology::PointList => todo!(),
            },
            Action::Flush => self.renderer.flush(),
        }
    }
}

fn worker(inner: Arc<Mutex<Inner>>, receiver: Receiver<Action>) {
    loop {
        let Ok(command) = receiver.recv() else {
            // sender has been dropped
            return;
        };

        {
            let mut renderer = inner.lock().unwrap();
            renderer.exec(command);

            while let Ok(action) = receiver.try_recv() {
                renderer.exec(action);
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
        let (sender, receiver) = channel();

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

        inner.blitter.blit(inner.renderer.viewport_view(), pass);
    }
}

impl RendererInterface for WgpuRenderer {
    fn exec(&mut self, action: Action) {
        self.sender.send(action).expect("rendering thread is alive");
    }
}
