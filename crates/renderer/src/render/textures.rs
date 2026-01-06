use std::collections::hash_map::Entry;

use rustc_hash::FxHashMap;

struct CachedTexture {
    texture: wgpu::Texture,
    generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextureHandle {
    pub id: u32,
    pub generation: u32,
}

pub struct Textures {
    cached: FxHashMap<u32, CachedTexture>,
    current: [TextureHandle; 8],
    textures: [wgpu::Texture; 8],
    samplers: [wgpu::Sampler; 8],
}

impl Textures {
    fn create_texture(device: &wgpu::Device, size: wgpu::Extent3d, label: &str) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
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
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        })
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let textures = std::array::from_fn(|i| {
            Self::create_texture(
                device,
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                &format!("default texture {i}"),
            )
        });
        let samplers = std::array::from_fn(|_| Self::create_sampler(device));

        Self {
            cached: FxHashMap::default(),
            current: [TextureHandle::default(); 8],
            textures,
            samplers,
        }
    }

    pub fn update_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: u32,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> TextureHandle {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = Self::create_texture(device, size, &format!("texture {id:08X}"));
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

        match self.cached.entry(id) {
            Entry::Occupied(mut o) => {
                let cached = o.get_mut();
                cached.texture = texture;
                cached.generation += 1;

                TextureHandle {
                    id,
                    generation: cached.generation,
                }
            }
            Entry::Vacant(v) => {
                v.insert(CachedTexture {
                    texture,
                    generation: 0,
                });

                TextureHandle { id, generation: 0 }
            }
        }
    }

    pub fn get_texture(&self, id: u32) -> Option<TextureHandle> {
        self.cached.get(&id).map(|c| TextureHandle {
            id,
            generation: c.generation,
        })
    }

    pub fn get_texture_slot(&self, index: usize) -> TextureHandle {
        self.current[index]
    }

    pub fn set_texture_slot(&mut self, index: usize, handle: TextureHandle) {
        self.current[index] = handle;
        self.textures[index] = self.cached.get(&handle.id).unwrap().texture.clone();
    }

    pub fn textures(&self) -> &[wgpu::Texture; 8] {
        &self.textures
    }

    pub fn samplers(&self) -> &[wgpu::Sampler; 8] {
        &self.samplers
    }
}
