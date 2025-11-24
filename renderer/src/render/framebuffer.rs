pub struct Framebuffer {
    /// Color component of the EFB.
    color: wgpu::Texture,
    /// Depth component of the EFB.
    depth: wgpu::Texture,
    /// Represents the external framebuffer.
    external: wgpu::Texture,
}

impl Framebuffer {
    fn create_textures(device: &wgpu::Device) -> (wgpu::Texture, wgpu::Texture, wgpu::Texture) {
        let size = wgpu::Extent3d {
            width: 640,
            height: 528,
            depth_or_array_layers: 1,
        };

        let external_tex = {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                dimension: wgpu::TextureDimension::D2,
                size,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
                mip_level_count: 1,
                sample_count: 1,
            })
        };

        let color_tex = {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                dimension: wgpu::TextureDimension::D2,
                size,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
                mip_level_count: 1,
                sample_count: 1,
            })
        };

        let depth_tex = {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                dimension: wgpu::TextureDimension::D2,
                size,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
                mip_level_count: 1,
                sample_count: 1,
            })
        };

        (external_tex, color_tex, depth_tex)
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let (external, color, depth) = Self::create_textures(device);

        Self {
            external,
            color,
            depth,
        }
    }

    pub fn external(&self) -> &wgpu::Texture {
        &self.external
    }

    pub fn color(&self) -> &wgpu::Texture {
        &self.color
    }

    pub fn depth(&self) -> &wgpu::Texture {
        &self.depth
    }
}
