mod blit;

use crate::blit::Blitter;
use hemisphere::render::{Action, Renderer};
use std::sync::{
    Arc, Mutex,
    mpsc::{Sender, channel},
};

struct Inner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    viewport: wgpu::Texture,
    viewport_view: wgpu::TextureView,

    blitter: Blitter,
}

impl Inner {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        // create viewport texture
        let viewport = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            dimension: wgpu::TextureDimension::D2,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 1,
        });

        let viewport_view = viewport.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            ..Default::default()
        });

        let blitter = Blitter::new(device.clone());
        Self {
            device,
            queue,
            viewport,
            viewport_view,

            blitter,
        }
    }

    fn exec(&mut self, action: Action) {
        // todo
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
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let inner = Arc::new(Mutex::new(Inner::new(device, queue)));
        let (sender, receiver) = channel();

        std::thread::Builder::new()
            .name("hemisphere wgpu renderer".into())
            .spawn({
                let inner = inner.clone();
                move || {
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

impl Renderer for WgpuRenderer {
    fn exec(&mut self, action: Action) {
        self.sender.send(action).expect("rendering thread is alive");
    }
}
