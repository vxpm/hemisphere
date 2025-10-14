mod blit;
mod render;

use crate::{blit::Blitter, render::Renderer};
use hemisphere::render::{Action, Renderer as RendererInterface, Viewport};
use std::sync::{
    Arc, Mutex,
    mpsc::{Receiver, Sender, channel},
};

struct Inner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    viewport: Viewport,
    viewport_tex: wgpu::Texture,
    viewport_view: wgpu::TextureView,
    blitter: Blitter,
    renderer: Renderer,
}

impl Inner {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
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

        let viewport_view = viewport_tex.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            ..Default::default()
        });

        let blitter = Blitter::new(device.clone(), format);
        let renderer = Renderer::new(device.clone());

        Self {
            device,
            queue,
            viewport: Viewport {
                width: 1,
                height: 1,
            },
            viewport_tex,
            viewport_view,
            blitter,
            renderer,
        }
    }

    pub fn resize_viewport(&mut self, viewport: Viewport) {
        if viewport == self.viewport {
            return;
        }

        tracing::info!(?viewport, "resizing viewport");
        let viewport_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
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

        let viewport_view = viewport_tex.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            ..Default::default()
        });

        self.viewport_tex = viewport_tex;
        self.viewport_view = viewport_view;
    }

    fn exec(&mut self, action: Action) {
        match action {
            Action::SetViewport(viewport) => {
                self.resize_viewport(viewport);
            }
            Action::SetVertexAttributes(vertex_attribute_set) => todo!(),
            Action::DrawTriangle(vertex_attributes) => todo!(),
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

        inner.blitter.blit(inner.viewport_view.clone(), pass);
    }
}

impl RendererInterface for WgpuRenderer {
    fn exec(&mut self, action: Action) {
        self.sender.send(action).expect("rendering thread is alive");
    }
}
