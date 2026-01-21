use std::collections::hash_map::Entry;

use lazuli::modules::render::Sampler;
use lazuli::system::gx::tex::WrapMode;
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct Cache {
    samplers: FxHashMap<Sampler, wgpu::Sampler>,
}

impl Cache {
    fn create_sampler(device: &wgpu::Device, sampler: Sampler) -> wgpu::Sampler {
        let address_mode = |wrap| match wrap {
            WrapMode::Clamp => wgpu::AddressMode::ClampToEdge,
            WrapMode::Repeat => wgpu::AddressMode::Repeat,
            WrapMode::Mirror => wgpu::AddressMode::MirrorRepeat,
            _ => panic!("reserved wrap mode"),
        };

        let mag_filter = if sampler.mode.mag_linear() {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        };

        let min_filter = if sampler.mode.min_filter().is_linear() {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        };

        let anisotropy_clamp = if sampler.mode.mag_linear() && sampler.mode.min_filter().is_linear()
        {
            16
        } else {
            1
        };

        device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: address_mode(sampler.mode.wrap_u()),
            address_mode_v: address_mode(sampler.mode.wrap_v()),
            mag_filter,
            min_filter,
            mipmap_filter: min_filter,
            anisotropy_clamp,
            lod_min_clamp: sampler.lods.min(),
            lod_max_clamp: sampler.lods.max(),
            ..Default::default()
        })
    }

    pub fn get(&mut self, device: &wgpu::Device, sampler: Sampler) -> &wgpu::Sampler {
        match self.samplers.entry(sampler) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let s = Self::create_sampler(device, sampler);
                v.insert(s)
            }
        }
    }
}
