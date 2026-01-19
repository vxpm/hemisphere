use std::collections::hash_map::Entry;

use lazuli::modules::render::{Clut, ClutId, Texture, TextureId};
use lazuli::system::gx::color::Rgba8;
use lazuli::system::gx::tex::{ClutFormat, TextureData};
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextureSettings {
    pub raw_id: TextureId,
    pub clut_id: ClutId,
    pub clut_fmt: ClutFormat,
}

struct WithDeps<T> {
    value: T,
    deps: FxHashSet<TextureSettings>,
}

const TMEM_LEN: usize = 1024 * 1024 / 2;

struct Tmem(Box<[u16; TMEM_LEN]>);

impl Default for Tmem {
    fn default() -> Self {
        Self(util::boxed_array(0))
    }
}

#[derive(Default)]
pub struct Cache {
    tmem: Tmem,
    raws: FxHashMap<TextureId, WithDeps<Texture>>,
    textures: FxHashMap<TextureSettings, wgpu::TextureView>,
}

impl Cache {
    fn create_texture_data_indirect(
        indirect: &Vec<u16>,
        palette: &[u16],
        format: ClutFormat,
    ) -> Vec<Rgba8> {
        let convert = match format {
            ClutFormat::IA8 => Rgba8::from_ia8,
            ClutFormat::RGB565 => Rgba8::from_rgb565,
            ClutFormat::RGB5A3 => Rgba8::from_rgb5a3,
            _ => panic!("reserved clut format"),
        };

        indirect
            .iter()
            .copied()
            .map(|index| {
                let color = palette[index as usize];
                convert(color)
            })
            .collect()
    }

    fn create_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        raws: &mut FxHashMap<TextureId, WithDeps<Texture>>,
        tmem: &mut Tmem,
        settings: TextureSettings,
    ) -> wgpu::TextureView {
        let raw = raws.get_mut(&settings.raw_id).unwrap();
        raw.deps.insert(settings);

        let owned_data;
        let data = match &raw.value.data {
            TextureData::Direct(data) => zerocopy::transmute_ref!(data.as_slice()),
            TextureData::Indirect(data) => {
                let create_addr = settings.clut_id.0 as usize * 16;
                let clut = &tmem.0[create_addr..];

                owned_data = Self::create_texture_data_indirect(&data, &clut, settings.clut_fmt);

                zerocopy::transmute_ref!(owned_data.as_slice())
            }
        };

        let size = wgpu::Extent3d {
            width: raw.value.width,
            height: raw.value.height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            dimension: wgpu::TextureDimension::D2,
            size,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 1,
        });

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
                bytes_per_row: Some(raw.value.width * 4),
                rows_per_image: None,
            },
            size,
        );

        texture.create_view(&Default::default())
    }

    pub fn update_raw(&mut self, id: TextureId, texture: Texture) {
        let old = self.raws.insert(
            id,
            WithDeps {
                value: texture,
                deps: Default::default(),
            },
        );

        if let Some(old) = old {
            for dep in old.deps.into_iter() {
                self.textures.remove(&dep);
            }
        }
    }

    pub fn update_clut(&mut self, id: ClutId, clut: Clut) {
        let addr = id.0 as usize * 16;

        // let mut current = addr;
        // for _ in 0..16 {
        //     self.tmem.0[current..][..clut.0.len()].copy_from_slice(&clut.0);
        //     current += clut.0.len();
        // }

        let mut current = addr;
        for entry in &clut.0 {
            for _ in 0..16 {
                self.tmem.0[current] = *entry;
                current += 1;
            }
        }
    }

    pub fn get(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        settings: TextureSettings,
    ) -> &wgpu::TextureView {
        match self.textures.entry(settings) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let texture =
                    Self::create_texture(device, queue, &mut self.raws, &mut self.tmem, settings);

                v.insert(texture)
            }
        }
    }
}
