#![feature(iter_array_chunks)]

mod render;
mod util;

use crate::{render::Renderer, util::blit::XfbBlitter};
use flume::{Receiver, Sender};
use hemisphere::modules::render::{Action, Renderer as RendererInterface};
use std::sync::{Arc, atomic::Ordering};

#[expect(clippy::needless_pass_by_value, reason = "makes it clearer")]
fn worker(mut renderer: Renderer, receiver: Receiver<Action>) {
    while let Ok(action) = receiver.recv() {
        renderer.exec(action);
    }
}

struct Inner {
    device: wgpu::Device,
    shared: Arc<render::Shared>,
    blitter: XfbBlitter,
}

/// A WGPU based renderer implementation.
///
/// This type is reference counted and therefore cheaply clonable.
#[derive(Clone)]
pub struct WgpuRenderer {
    inner: Arc<Inner>,
    sender: Sender<Action>,
}

impl WgpuRenderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let blitter = XfbBlitter::new(&device, format);
        let (renderer, shared) = Renderer::new(device.clone(), queue);
        let (sender, receiver) = flume::bounded(4096);

        std::thread::Builder::new()
            .name("hemisphere wgpu renderer".into())
            .spawn(move || worker(renderer, receiver))
            .unwrap();

        Self {
            inner: Arc::new(Inner {
                device,
                shared,
                blitter,
            }),
            sender,
        }
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        let xfb = self.inner.shared.xfb.lock().unwrap();
        self.inner.blitter.blit_to_target(
            &self.inner.device,
            &xfb,
            wgpu::Origin3d::ZERO,
            wgpu::Extent3d {
                width: 640,
                height: 528,
                depth_or_array_layers: 1,
            },
            pass,
        );
    }

    pub fn rendered_anything(&self) -> bool {
        self.inner
            .shared
            .rendered_anything
            .swap(false, Ordering::SeqCst)
    }
}

impl RendererInterface for WgpuRenderer {
    fn exec(&mut self, action: Action) {
        self.sender.send(action).expect("rendering thread is alive");
    }
}
