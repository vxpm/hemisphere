use crate::system::gpu::colors::Rgba8;
use bitos::{
    bitos,
    integer::{u2, u10},
};
use gekko::Address;
use rustc_hash::FxBuildHasher;
use std::collections::HashMap;

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapMode {
    #[default]
    Clamp = 0x0,
    Repeat = 0x1,
    Mirror = 0x2,
    Reserved = 0x3,
}

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MinFilter {
    #[default]
    Near = 0x0,
    NearMipNear = 0x1,
    NearMipLinear = 0x2,
    Reserved0 = 0x3,
    Linear = 0x4,
    LinearMipNear = 0x5,
    LinearMipLinear = 0x6,
    Reserved = 0x7,
}

#[bitos(4)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataFormat {
    #[default]
    Intensity4 = 0x0,
    Intensity8 = 0x1,
    Intensity4Alpha = 0x2,
    Intensity8Alpha = 0x3,
    Rgb565 = 0x4,
    Rgb5A3 = 0x5,
    Rgba8 = 0x6,
    Reserved0 = 0x7,
    C4 = 0x8,
    C8 = 0x9,
    C14X2 = 0xA,
    Reserved1 = 0xB,
    Reserved2 = 0xC,
    Reserved3 = 0xD,
    Cmp = 0xE,
    Reserved4 = 0xF,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Mode {
    #[bits(0..2)]
    pub wrap_s: WrapMode,
    #[bits(2..4)]
    pub wrap_t: WrapMode,
    #[bits(4)]
    pub mag_linear: bool,
    #[bits(5..8)]
    pub min_filter: MinFilter,
    #[bits(8)]
    pub diagonal_lod: bool,
    #[bits(9..17)]
    pub lod_bias: u8,
    #[bits(19..21)]
    pub max_anisotropy_log2: u2,
    #[bits(21)]
    pub lod_and_bias_clamp: bool,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Format {
    #[bits(0..10)]
    pub width_minus_one: u10,
    #[bits(10..20)]
    pub height_minus_one: u10,
    #[bits(20..24)]
    pub data_format: DataFormat,
}

impl Format {
    pub fn width(&self) -> u32 {
        self.width_minus_one().value() as u32 + 1
    }

    pub fn height(&self) -> u32 {
        self.height_minus_one().value() as u32 + 1
    }

    // Size, in bytes, of the texture.
    pub fn size(&self) -> u32 {
        let pixels = |n| self.width().next_multiple_of(n) * self.height().next_multiple_of(n);
        let pixels_ab = |a, b| self.width().next_multiple_of(a) * self.height().next_multiple_of(b);

        match self.data_format() {
            DataFormat::Intensity4 => pixels(8) / 2,
            DataFormat::Intensity8 => pixels_ab(8, 4),
            DataFormat::Intensity4Alpha => pixels(8),
            DataFormat::Intensity8Alpha => pixels(4) * 2,
            DataFormat::Rgb565 => pixels(4) * 2,
            DataFormat::Rgb5A3 => pixels(4) * 2,
            DataFormat::Rgba8 => pixels(4) * 4,
            DataFormat::Cmp => pixels(8) / 2,
            DataFormat::C8 => pixels(1),
            DataFormat::C4 => pixels(1) / 2,
            _ => todo!("format {:?}", self.data_format()),
        }
    }
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ScaleS {
    #[bits(0..16)]
    pub scale_minus_one: u16,
    #[bits(16)]
    pub range_bias_enable: bool,
    #[bits(17)]
    pub cylindrical_wrapping: bool,
    #[bits(18)]
    pub offset_lines: bool,
    #[bits(19)]
    pub offset_points: bool,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ScaleT {
    #[bits(0..16)]
    pub scale_minus_one: u16,
    #[bits(16)]
    pub range_bias_enable: bool,
    #[bits(17)]
    pub cylindrical_wrapping: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Scaling {
    pub s: ScaleS,
    pub t: ScaleT,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TextureMap {
    pub address: Address,
    pub format: Format,
    pub mode: Mode,
    pub scaling: Scaling,
    pub dirty: bool,
}

#[derive(Default)]
pub struct Interface {
    pub maps: [TextureMap; 8],
    pub cache: HashMap<Address, u64>,
    pub hasher: FxBuildHasher,
}

impl std::fmt::Debug for Interface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interface")
            .field("maps", &self.maps)
            .field("cache", &self.cache)
            .finish()
    }
}

impl Interface {
    /// Given an address and the texture data present there, returns whether the data hash matches
    /// with the one in the cache. If not, the hash is inserted into the cache.
    pub fn insert_cache(&mut self, addr: Address, data: &[u8]) -> bool {
        use std::hash::BuildHasher;
        let new_hash = self.hasher.hash_one(data);
        if let Some(old_hash) = self.cache.get(&addr) {
            if *old_hash == new_hash {
                true
            } else {
                self.cache.insert(addr, new_hash);
                false
            }
        } else {
            self.cache.insert(addr, new_hash);
            false
        }
    }
}

pub fn decode_texture(data: &[u8], format: Format) -> Vec<Rgba8> {
    let width = format.width() as usize;
    let height = format.height() as usize;
    let pixels = match format.data_format() {
        DataFormat::Intensity4 => gxtex::decode::<gxtex::Intensity4>(width, height, data),
        DataFormat::Intensity4Alpha => gxtex::decode::<gxtex::Intensity4Alpha>(width, height, data),
        DataFormat::Intensity8 => gxtex::decode::<gxtex::Intensity8>(width, height, data),
        DataFormat::Intensity8Alpha => gxtex::decode::<gxtex::Intensity8Alpha>(width, height, data),
        DataFormat::Rgb565 => gxtex::decode::<gxtex::Rgb565>(width, height, data),
        DataFormat::Rgb5A3 => gxtex::decode::<gxtex::Rgb5A3>(width, height, data),
        DataFormat::Rgba8 => gxtex::decode::<gxtex::Rgba8>(width, height, data),
        DataFormat::Cmp => gxtex::decode::<gxtex::Cmpr>(width, height, data),
        DataFormat::C8 | DataFormat::C4 => {
            vec![
                gxtex::Pixel {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 128
                };
                (format.width() * format.height()) as usize
            ]
        }
        _ => todo!("format {format:?}"),
    };

    pixels
        .into_iter()
        .map(|c| Rgba8 {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        })
        .collect()
}

/// Stride should be in bytes.
pub fn encode_texture(data: Vec<Rgba8>, format: Format, stride: u32, output: &mut [u8]) {
    match format.data_format() {
        _ => todo!("format {format:?}"),
    }
}
