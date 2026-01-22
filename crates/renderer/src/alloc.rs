//! Buffer allocator.

use flume::{Receiver, Sender};

#[derive(Clone)]
struct BufferPair {
    main: wgpu::Buffer,
    staging: wgpu::Buffer,
}

pub struct Allocator {
    usages: wgpu::BufferUsages,
    available: Vec<BufferPair>,
    allocated: Vec<BufferPair>,
    sender: Sender<BufferPair>,
    receiver: Receiver<BufferPair>,
}

impl Allocator {
    pub fn new(usages: wgpu::BufferUsages) -> Self {
        let (sender, receiver) = flume::unbounded();
        Self {
            usages,
            available: Default::default(),
            allocated: Default::default(),
            sender,
            receiver,
        }
    }

    fn recall(&mut self) {
        if self.receiver.is_empty() {
            return;
        }

        while let Ok(pair) = self.receiver.try_recv() {
            self.available.push(pair);
        }

        self.available.sort_unstable_by_key(|b| b.main.size());
    }

    pub fn allocate(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        data: &[u8],
    ) -> wgpu::Buffer {
        self.recall();

        let size = data.len() as u64;
        let to_remove = self.available.partition_point(|b| b.main.size() < size);
        let pair = (to_remove < self.available.len()).then(|| self.available.remove(to_remove));

        let pair = match pair {
            Some(pair) => pair,
            None => {
                let buffer_size = size.next_multiple_of(256);
                let main = device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: buffer_size,
                    usage: self.usages | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });

                let staging = device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: buffer_size,
                    usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: true,
                });

                BufferPair { main, staging }
            }
        };

        {
            let mut mapped = pair.staging.get_mapped_range_mut(..size);
            mapped.copy_from_slice(data);
        }

        pair.staging.unmap();
        encoder.copy_buffer_to_buffer(&pair.staging, 0, &pair.main, 0, Some(size));

        let buffer = pair.main.clone();
        self.allocated.push(pair);

        buffer
    }

    pub fn free(&mut self) {
        for pair in self.allocated.drain(..) {
            let sender = self.sender.clone();
            let staging = pair.staging.clone();

            staging.map_async(wgpu::MapMode::Write, .., move |_| {
                sender.send(pair).unwrap()
            });
        }
    }
}
