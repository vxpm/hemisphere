use std::{collections::BTreeMap, ops::Bound};
use wgpu::util::DeviceExt;

pub struct Buffers {
    usages: wgpu::BufferUsages,
    allocated: Vec<wgpu::Buffer>,
    free: BTreeMap<u64, wgpu::Buffer>,
}

impl Buffers {
    pub fn new(usages: wgpu::BufferUsages) -> Self {
        Self {
            usages,
            allocated: Default::default(),
            free: Default::default(),
        }
    }

    pub fn allocate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
    ) -> wgpu::Buffer {
        let size = data.len() as u64;
        let mut gap = self.free.lower_bound_mut(Bound::Included(&size));
        match gap.remove_next() {
            Some((_, buffer)) => {
                queue.write_buffer(&buffer, 0, data);
                queue.submit([]);

                self.allocated.push(buffer.clone());
                buffer
            }
            None => {
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: data,
                    usage: self.usages | wgpu::BufferUsages::COPY_DST,
                });
                self.allocated.push(buffer.clone());
                buffer
            }
        }
    }

    pub fn recall(&mut self) {
        self.free
            .extend(self.allocated.drain(..).map(|b| (b.size() as u64, b)))
    }
}
