//! Framebuffer (EFB color, EFB depth, XFB).

use lazuli::system::gx::{EFB_HEIGHT, EFB_WIDTH};

pub struct Framebuffer {
    /// Color component of the EFB.
    color: wgpu::TextureView,
    /// Multisampled color component of the EFB.
    multisampled_color: wgpu::TextureView,
    /// Depth component of the EFB.
    depth: wgpu::TextureView,
    /// Represents the external framebuffer.
    external: wgpu::TextureView,
}

impl Framebuffer {
    pub fn new(device: &wgpu::Device) -> Self {
        let size = wgpu::Extent3d {
            width: EFB_WIDTH as u32,
            height: EFB_HEIGHT as u32,
            depth_or_array_layers: 1,
        };

        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("efb color resolved"),
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

        let color = color.create_view(&Default::default());
        let multisampled_color = multisampled_color.create_view(&Default::default());
        let depth = depth.create_view(&Default::default());
        let external = external.create_view(&Default::default());

        Self {
            color,
            multisampled_color,
            depth,
            external,
        }
    }

    pub fn external(&self) -> &wgpu::TextureView {
        &self.external
    }

    pub fn color(&self) -> &wgpu::TextureView {
        &self.color
    }

    pub fn multisampled_color(&self) -> &wgpu::TextureView {
        &self.multisampled_color
    }

    pub fn depth(&self) -> &wgpu::TextureView {
        &self.depth
    }
}
