#![feature(iter_array_chunks)]

mod blit;
mod render;

use crate::{blit::Blitter, render::Renderer};
use flume::{Receiver, Sender};
use hemisphere::render::{Action, Renderer as RendererInterface};
use std::sync::{Arc, atomic::Ordering};

#[expect(clippy::needless_pass_by_value, reason = "makes it clearer")]
fn worker(mut renderer: Renderer, receiver: Receiver<Action>) {
    while let Ok(action) = receiver.recv() {
        renderer.exec(action);
    }
}

struct Inner {
    device: wgpu::Device,
    blitter: Blitter,
    shared: Arc<render::Shared>,
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
        let blitter = Blitter::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb, format);
        let (renderer, shared) = Renderer::new(device.clone(), queue);
        let (sender, receiver) = flume::bounded(4096);

        std::thread::Builder::new()
            .name("hemisphere wgpu renderer".into())
            .spawn(move || worker(renderer, receiver))
            .unwrap();

        Self {
            inner: Arc::new(Inner {
                device,
                blitter,
                shared,
            }),
            sender,
        }
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        let xfb = self.inner.shared.xfb.lock().unwrap();
        self.inner.blitter.blit(&self.inner.device, &xfb, pass);
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
