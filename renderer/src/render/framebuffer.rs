pub struct Framebuffer {
    /// Color component of the EFB.
    color: wgpu::Texture,
    /// Multisampled color component of the EFB.
    multisampled_color: wgpu::Texture,
    /// Depth component of the EFB.
    depth: wgpu::Texture,
    /// Represents the external framebuffer.
    external: wgpu::Texture,
}

impl Framebuffer {
    pub fn new(device: &wgpu::Device) -> Self {
        let size = wgpu::Extent3d {
            width: 640,
            height: 528,
            depth_or_array_layers: 1,
        };

        let external = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("xfb"),
            dimension: wgpu::TextureDimension::D2,
            size,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 1,
        });

        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("efb color"),
            dimension: wgpu::TextureDimension::D2,
            size,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 1,
        });

        let multisampled_color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("efb color multisampled"),
            dimension: wgpu::TextureDimension::D2,
            size,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 4,
        });

        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("efb depth"),
            dimension: wgpu::TextureDimension::D2,
            size,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 4,
        });

        Self {
            external,
            color,
            multisampled_color,
            depth,
        }
    }

    pub fn external(&self) -> &wgpu::Texture {
        &self.external
    }

    pub fn color(&self) -> &wgpu::Texture {
        &self.color
    }

    pub fn multisampled_color(&self) -> &wgpu::Texture {
        &self.multisampled_color
    }

    pub fn depth(&self) -> &wgpu::Texture {
        &self.depth
    }
}
