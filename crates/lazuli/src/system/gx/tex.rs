//! Texture unit (TX).
use crate::system::gx::{
    colors::Rgba8,
    pix::{ColorCopyFormat, DepthCopyFormat},
};
use bitos::{
    bitos,
    integer::{u2, u10, u11},
};
use gekko::Address;
use gxtex::PaletteIndex;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum TextureData {
    Direct(Vec<Rgba8>),
    Indirect(Vec<PaletteIndex>),
}

impl TextureData {
    fn direct(data: Vec<gxtex::Pixel>) -> Self {
        Self::Direct(
            data.into_iter()
                .map(|p| Rgba8 {
                    r: p.r,
                    g: p.g,
                    b: p.b,
                    a: p.a,
                })
                .collect(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub data: TextureData,
}

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
pub enum Format {
    #[default]
    I4 = 0x0,
    I8 = 0x1,
    IA4 = 0x2,
    IA8 = 0x3,
    Rgb565 = 0x4,
    Rgb5A3 = 0x5,
    Rgba8 = 0x6,
    Reserved0 = 0x7,
    CI4 = 0x8,
    CI8 = 0x9,
    CI14X2 = 0xA,
    Reserved1 = 0xB,
    Reserved2 = 0xC,
    Reserved3 = 0xD,
    Cmp = 0xE,
    Reserved4 = 0xF,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Sampler {
    #[bits(0..2)]
    pub wrap_u: WrapMode,
    #[bits(2..4)]
    pub wrap_v: WrapMode,
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
pub struct Encoding {
    #[bits(0..10)]
    pub width_minus_one: u10,
    #[bits(10..20)]
    pub height_minus_one: u10,
    #[bits(20..24)]
    pub format: Format,
}

impl Encoding {
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

        match self.format() {
            Format::I4 => pixels(8) / 2,
            Format::I8 => pixels_ab(8, 4),
            Format::IA4 => pixels(8),
            Format::IA8 => pixels(4) * 2,
            Format::Rgb565 => pixels(4) * 2,
            Format::Rgb5A3 => pixels(4) * 2,
            Format::Rgba8 => pixels(4) * 4,
            Format::Cmp => pixels(8) / 2,
            Format::CI8 => pixels(1),
            Format::CI4 => pixels(1) / 2,
            _ => todo!("format {:?}", self.format()),
        }
    }
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ScaleU {
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
pub struct ScaleV {
    #[bits(0..16)]
    pub scale_minus_one: u16,
    #[bits(16)]
    pub range_bias_enable: bool,
    #[bits(17)]
    pub cylindrical_wrapping: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Scaling {
    pub u: ScaleU,
    pub v: ScaleV,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TextureMap {
    pub address: Address,
    pub format: Encoding,
    pub sampler: Sampler,
    pub scaling: Scaling,
    pub lut: LutRef,
    pub dirty: bool,
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LutFormat {
    #[default]
    IA8 = 0b00,
    RGB565 = 0b01,
    RGB5A3 = 0b10,
    Reserved0 = 0b11,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct LutCount {
    #[bits(0..10)]
    pub tmem_offset: u10,
    #[bits(10..21)]
    pub count: u11,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct LutRef {
    #[bits(0..10)]
    pub tmem_offset: u10,
    #[bits(10..12)]
    pub format: LutFormat,
}

#[derive(Default)]
pub struct Interface {
    pub maps: [TextureMap; 8],
    pub cache: HashMap<Address, u64>,
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
        let new_hash = twox_hash::XxHash3_64::oneshot(data);
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

pub fn decode_texture(data: &[u8], format: Encoding) -> TextureData {
    use gxtex::{
        AlphaChannel, CI4, CI8, CI14X2, Cmpr, FastLuma, FastRgb565, I4, I8, IA4, IA8, Rgb5A3,
        Rgba8, decode,
    };

    let width = format.width() as usize;
    let height = format.height() as usize;

    match format.format() {
        Format::I4 => TextureData::direct(decode::<I4<FastLuma>>(width, height, data)),
        Format::IA4 => {
            TextureData::direct(decode::<IA4<FastLuma, AlphaChannel>>(width, height, data))
        }
        Format::I8 => TextureData::direct(decode::<I8<FastLuma>>(width, height, data)),
        Format::IA8 => {
            TextureData::direct(decode::<IA8<FastLuma, AlphaChannel>>(width, height, data))
        }
        Format::Rgb565 => TextureData::direct(decode::<FastRgb565>(width, height, data)),
        Format::Rgb5A3 => TextureData::direct(decode::<Rgb5A3>(width, height, data)),
        Format::Rgba8 => TextureData::direct(decode::<Rgba8>(width, height, data)),
        Format::Cmp => TextureData::direct(decode::<Cmpr>(width, height, data)),
        Format::CI4 => TextureData::Indirect(decode::<CI4>(width, height, data)),
        Format::CI8 => TextureData::Indirect(decode::<CI8>(width, height, data)),
        Format::CI14X2 => TextureData::Indirect(decode::<CI14X2>(width, height, data)),
        _ => todo!("reserved texture format"),
    }
}

pub fn encode_color_texture(
    data: Vec<Rgba8>,
    format: ColorCopyFormat,
    stride: u32,
    width: u32,
    height: u32,
    output: &mut [u8],
) {
    use gxtex::{
        AlphaChannel, BlueChannel, FastLuma, FastRgb565, GreenChannel, I4, I8, IA4, IA8,
        RedChannel, Rgb5A3, Rgba8, encode,
    };

    let pixels = data
        .into_iter()
        .map(|c| gxtex::Pixel {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        })
        .collect::<Vec<_>>();

    macro_rules! encode {
        ($fmt:ty) => {
            encode::<$fmt>(
                stride as usize,
                width as usize,
                height as usize,
                &pixels,
                output,
            )
        };
    }

    match format {
        ColorCopyFormat::R4 => encode!(I4<RedChannel>),
        ColorCopyFormat::Y8 => encode!(I8<FastLuma>),
        ColorCopyFormat::RA4 => encode!(IA4<RedChannel, AlphaChannel>),
        ColorCopyFormat::RA8 => encode!(IA8<RedChannel, AlphaChannel>),
        ColorCopyFormat::RGB565 => encode!(FastRgb565),
        ColorCopyFormat::RGB5A3 => encode!(Rgb5A3),
        ColorCopyFormat::RGBA8 => encode!(Rgba8),
        ColorCopyFormat::A8 => encode!(I8<AlphaChannel>),
        ColorCopyFormat::R8 => encode!(I8<RedChannel>),
        ColorCopyFormat::G8 => encode!(I8<GreenChannel>),
        ColorCopyFormat::B8 => encode!(I8<BlueChannel>),
        ColorCopyFormat::RG8 => encode!(IA8<RedChannel, GreenChannel>),
        ColorCopyFormat::GB8 => encode!(IA8<GreenChannel, BlueChannel>),
        _ => panic!("reserved color format"),
    }
}

pub fn encode_depth_texture(
    data: Vec<u32>,
    format: DepthCopyFormat,
    stride: u32,
    width: u32,
    height: u32,
    output: &mut [u8],
) {
    use gxtex::{BlueChannel, GreenChannel, I8, IA8, RedChannel, Rgba8, encode};

    let depth = data
        .into_iter()
        .map(u32::to_le_bytes)
        .map(|c| gxtex::Pixel {
            r: c[0], // low
            g: c[1], // mid
            b: c[2], // high
            a: 0,
        })
        .collect::<Vec<_>>();

    macro_rules! encode {
        ($fmt:ty) => {
            encode::<$fmt>(
                stride as usize,
                width as usize,
                height as usize,
                &depth,
                output,
            )
        };
    }

    match format {
        DepthCopyFormat::Z4 => todo!(),
        DepthCopyFormat::Z8 => encode!(I8<RedChannel>), // not sure...
        DepthCopyFormat::Z16C => encode!(IA8<RedChannel, GreenChannel>),
        DepthCopyFormat::Z24X8 => encode!(Rgba8),
        DepthCopyFormat::Z8H => encode!(I8<BlueChannel>),
        DepthCopyFormat::Z8M => encode!(I8<GreenChannel>),
        DepthCopyFormat::Z8L => encode!(I8<RedChannel>),
        DepthCopyFormat::Z16A => encode!(IA8<GreenChannel, RedChannel>),
        DepthCopyFormat::Z16B => encode!(IA8<GreenChannel, RedChannel>),
        _ => panic!("reserved depth format"),
    }
}
