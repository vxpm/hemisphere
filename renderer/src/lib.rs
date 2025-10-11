use std::sync::{
    Arc, Mutex,
    mpsc::{Sender, channel},
};

use hemisphere::render::{Action, Renderer};

struct Inner {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Inner {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self { device, queue }
    }

    fn exec(&mut self, action: Action) {
        todo!()
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
        todo!()
    }
}

impl Renderer for WgpuRenderer {
    fn exec(&mut self, action: Action) {
        self.sender.send(action).expect("rendering thread is alive");
    }
}
