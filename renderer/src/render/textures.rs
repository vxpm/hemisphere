pub struct Textures {
    textures: [wgpu::Texture; 8],
    samplers: [wgpu::Sampler; 8],
}

impl Textures {
    fn create_texture(device: &wgpu::Device, size: wgpu::Extent3d) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            dimension: wgpu::TextureDimension::D2,
            size,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 1,
        })
    }

    fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        })
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let textures = std::array::from_fn(|_| {
            Self::create_texture(
                device,
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            )
        });
        let samplers = std::array::from_fn(|_| Self::create_sampler(device));

        Self { textures, samplers }
    }

    pub fn update_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        index: usize,
        width: u32,
        height: u32,
        data: &[u8],
    ) {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = Self::create_texture(device, size);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::default(),
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: None,
            },
            size,
        );

        self.textures[index] = texture;
    }

    pub fn textures(&self) -> &[wgpu::Texture; 8] {
        &self.textures
    }

    pub fn samplers(&self) -> &[wgpu::Sampler; 8] {
        &self.samplers
    }
}
