//! Buffer allocator.

use flume::{Receiver, Sender};

#[derive(Clone)]
struct BufferPair {
    main: wgpu::Buffer,
    staging: wgpu::Buffer,
}

pub struct Allocator {
    usages: wgpu::BufferUsages,
    available: Vec<Vec<BufferPair>>,
    allocated: Vec<BufferPair>,
    sender: Sender<BufferPair>,
    receiver: Receiver<BufferPair>,
}

fn bucket_for(size: u64) -> usize {
    size.ilog2() as usize - 4
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
            let size = pair.main.size();
            let bucket = bucket_for(size);

            if self.available.len() <= bucket {
                self.available.resize(bucket + 1, Vec::new());
            }

            self.available[bucket].push(pair);
        }
    }

    fn allocate_inner(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        data: &[u8],
        recall: bool,
    ) -> wgpu::Buffer {
        let size = data.len() as u64;
        let buffer_size = size.next_power_of_two().max(16);
        let bucket = bucket_for(buffer_size);

        let pair = self
            .available
            .get_mut(bucket)
            .and_then(|bucket| bucket.pop());

        let pair = match pair {
            Some(pair) => pair,
            None => {
                if recall {
                    self.recall();
                    return self.allocate_inner(device, encoder, data, false);
                }

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

    pub fn allocate(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        data: &[u8],
    ) -> wgpu::Buffer {
        self.allocate_inner(device, encoder, data, true)
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
