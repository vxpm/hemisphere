use hemisphere::render::Viewport;
use wgpu::util::DeviceExt;

pub struct Textures {
    pub color: wgpu::Texture,
    pub depth: wgpu::Texture,
}

pub struct Framebuffer {
    viewport: Viewport,
    textures: [Textures; 2],
    front_is_second: bool,
}

impl Framebuffer {
    fn create_textures(device: &wgpu::Device, viewport: Viewport) -> [Textures; 2] {
        let size = wgpu::Extent3d {
            width: viewport.width.max(1),
            height: viewport.height.max(1),
            depth_or_array_layers: 1,
        };

        let color_tex = || {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                dimension: wgpu::TextureDimension::D2,
                size,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
                mip_level_count: 1,
                sample_count: 1,
            })
        };

        let depth_tex = || {
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

        let tex = || Textures {
            color: color_tex(),
            depth: depth_tex(),
        };

        [tex(), tex()]
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let viewport = Viewport {
            width: 1,
            height: 1,
        };
        let textures = Self::create_textures(device, viewport);

        Self {
            viewport,
            textures,
            front_is_second: false,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, viewport: Viewport) {
        if viewport == self.viewport {
            return;
        }

        tracing::info!(?viewport, "resizing viewport");
        self.viewport = viewport;
        self.textures = Self::create_textures(device, viewport);
    }

    pub fn front(&self) -> &Textures {
        &self.textures[self.front_is_second as usize]
    }

    pub fn back(&self) -> &Textures {
        &self.textures[!self.front_is_second as usize]
    }

    pub fn swap(&mut self) {
        self.front_is_second = !self.front_is_second;
    }
}
