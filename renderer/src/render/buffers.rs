pub struct Buffers {
    usages: wgpu::BufferUsages,
    allocated: Vec<wgpu::Buffer>,
    free: Vec<wgpu::Buffer>,
}

impl Buffers {
    pub fn new(usages: wgpu::BufferUsages) -> Self {
        Self {
            usages,
            allocated: Default::default(),
            free: Default::default(),
        }
    }

    #[expect(dead_code, reason = "will be used later")]
    pub fn count(&self) -> usize {
        self.free.len() + self.allocated.len()
    }

    #[expect(dead_code, reason = "will be used later")]
    pub fn total_size(&self) -> u64 {
        self.free
            .iter()
            .map(|b| b.size())
            .chain(self.allocated.iter().map(|b| b.size()))
            .sum()
    }

    pub fn allocate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
    ) -> wgpu::Buffer {
        let size = data.len() as u64;
        let to_remove = self.free.partition_point(|b| b.size() < size);
        let buffer = (to_remove < self.free.len()).then(|| self.free.remove(to_remove));

        match buffer {
            Some(buffer) => {
                queue.write_buffer(&buffer, 0, data);
                queue.submit([]);
                self.allocated.push(buffer.clone());
                buffer
            }
            None => {
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: size.next_multiple_of(256),
                    usage: self.usages | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: true,
                });

                let mut mapped = buffer.get_mapped_range_mut(..data.len() as u64);
                mapped.copy_from_slice(data);
                std::mem::drop(mapped);

                buffer.unmap();

                self.allocated.push(buffer.clone());
                buffer
            }
        }
    }

    pub fn recall(&mut self) {
        self.free.extend(self.allocated.drain(..));
        self.free.sort_unstable_by_key(|b| b.size());
    }
}
