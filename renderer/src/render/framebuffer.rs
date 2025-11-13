use hemisphere::render::Viewport;

pub struct Framebuffer {
    viewport: Viewport,
    front: wgpu::Texture,
    color: wgpu::Texture,
    depth: wgpu::Texture,
}

impl Framebuffer {
    fn create_textures(
        device: &wgpu::Device,
        viewport: Viewport,
    ) -> (wgpu::Texture, wgpu::Texture, wgpu::Texture) {
        let size = wgpu::Extent3d {
            width: viewport.width.max(1),
            height: viewport.height.max(1),
            depth_or_array_layers: 1,
        };

        let front_tex = {
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

        (front_tex, color_tex, depth_tex)
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let viewport = Viewport {
            width: 1,
            height: 1,
        };

        let (front, color, depth) = Self::create_textures(device, viewport);

        Self {
            viewport,
            front,
            color,
            depth,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, viewport: Viewport) -> bool {
        if viewport == self.viewport {
            return false;
        }

        tracing::info!(?viewport, "resizing viewport");
        let (front, color, depth) = Self::create_textures(device, viewport);
        self.front = front;
        self.color = color;
        self.depth = depth;
        self.viewport = viewport;

        true
    }

    pub fn front(&self) -> &wgpu::Texture {
        &self.front
    }

    pub fn color(&self) -> &wgpu::Texture {
        &self.color
    }

    pub fn depth(&self) -> &wgpu::Texture {
        &self.depth
    }
}
