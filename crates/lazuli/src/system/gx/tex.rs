//! Texture unit (TX).
use std::collections::HashMap;

use bitos::integer::{u2, u10, u11};
use bitos::{BitUtils, bitos};
use color::Rgba8;
use gekko::Address;
use gxtex::PaletteIndex;

use crate::modules::render;
use crate::system::System;
use crate::system::gx::pix::{ColorCopyFormat, DepthCopyFormat};

#[derive(Debug, Clone)]
pub enum TextureData {
    Direct(Vec<Rgba8>),
    Indirect(Vec<PaletteIndex>),
}

#[derive(Debug, Clone)]
pub enum MipmapData {
    Direct(Vec<Vec<Rgba8>>),
    Indirect(Vec<Vec<PaletteIndex>>),
}

impl MipmapData {
    fn push(&mut self, lod: TextureData) {
        match (self, lod) {
            (Self::Direct(lods), TextureData::Direct(lod)) => lods.push(lod),
            (Self::Indirect(lods), TextureData::Indirect(lod)) => lods.push(lod),
            _ => panic!("mismatched mipmap and texture formats - this is definitely a bug"),
        }
    }

    pub fn lod_count(&self) -> u32 {
        match self {
            Self::Direct(lods) => lods.len() as u32,
            Self::Indirect(lods) => lods.len() as u32,
        }
    }
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapMode {
    #[default]
    Clamp    = 0x0,
    Repeat   = 0x1,
    Mirror   = 0x2,
    Reserved = 0x3,
}

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MinFilter {
    #[default]
    Near            = 0x0,
    NearMipNear     = 0x1,
    NearMipLinear   = 0x2,
    Reserved0       = 0x3,
    Linear          = 0x4,
    LinearMipNear   = 0x5,
    LinearMipLinear = 0x6,
    Reserved        = 0x7,
}

impl MinFilter {
    pub fn is_linear(&self) -> bool {
        matches!(
            self,
            Self::Linear | Self::LinearMipNear | Self::LinearMipLinear
        )
    }

    pub fn uses_lods(&self) -> bool {
        matches!(
            self,
            Self::NearMipNear | Self::NearMipLinear | Self::LinearMipNear | Self::LinearMipLinear
        )
    }
}

#[bitos(4)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    I4        = 0x0,
    I8        = 0x1,
    IA4       = 0x2,
    IA8       = 0x3,
    Rgb565    = 0x4,
    Rgb5A3    = 0x5,
    Rgba8     = 0x6,
    Reserved0 = 0x7,
    CI4       = 0x8,
    CI8       = 0x9,
    CI14X2    = 0xA,
    Reserved1 = 0xB,
    Reserved2 = 0xC,
    Reserved3 = 0xD,
    Cmp       = 0xE,
    Reserved4 = 0xF,
}

impl Format {
    pub fn is_direct(&self) -> bool {
        !matches!(self, Self::CI4 | Self::CI8 | Self::CI14X2)
    }
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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

    pub fn lod_count(&self) -> u32 {
        self.width().ilog2().min(self.height().ilog2()) + 1
    }

    pub fn length_for(width: u32, height: u32, format: Format) -> u32 {
        let pixels = |n| width.next_multiple_of(n) * height.next_multiple_of(n);
        let pixels_xy = |x, y| width.next_multiple_of(x) * height.next_multiple_of(y);

        match format {
            Format::I4 => pixels(8) / 2,
            Format::I8 => pixels_xy(8, 4),
            Format::IA4 => pixels(8),
            Format::IA8 => pixels(4) * 2,
            Format::Rgb565 => pixels(4) * 2,
            Format::Rgb5A3 => pixels(4) * 2,
            Format::Rgba8 => pixels(4) * 4,
            Format::Cmp => pixels(8) / 2,
            Format::CI8 => pixels(1),
            Format::CI4 => pixels(1) / 2,
            _ => todo!("format {:?}", format),
        }
    }

    // Size, in bytes, of the texture.
    pub fn length(&self) -> u32 {
        Self::length_for(self.width(), self.height(), self.format())
    }

    // Size, in bytes, of the mipmap.
    pub fn length_mipmap(&self) -> u32 {
        let mut current_width = self.width();
        let mut current_height = self.height();

        let mut size = 0;
        for _ in 0..self.lod_count() {
            size += Self::length_for(current_width, current_height, self.format());
            current_width /= 2;
            current_height /= 2;
        }

        size
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
    pub encoding: Encoding,
    pub sampler: Sampler,
    pub scaling: Scaling,
    pub clut: LutRef,
    pub dirty: bool,
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClutFormat {
    #[default]
    IA8       = 0b00,
    RGB565    = 0b01,
    RGB5A3    = 0b10,
    Reserved0 = 0b11,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ClutLoad {
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
    pub format: ClutFormat,
}

#[derive(Default)]
pub struct Interface {
    pub maps: [TextureMap; 8],
    pub clut_base: u32,
    pub clut_load: ClutLoad,
    pub tex_cache: HashMap<Address, u64>,
    pub clut_cache: HashMap<Address, u64>,
}

impl std::fmt::Debug for Interface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interface")
            .field("maps", &self.maps)
            .field("cache", &self.tex_cache)
            .finish()
    }
}

impl Interface {
    pub fn is_tex_dirty(&mut self, addr: Address, data: &[u8]) -> bool {
        let new_hash = twox_hash::XxHash3_64::oneshot(data);
        let Some(old_hash) = self.tex_cache.get(&addr) else {
            self.tex_cache.insert(addr, new_hash);
            return false;
        };

        if *old_hash == new_hash {
            true
        } else {
            self.tex_cache.insert(addr, new_hash);
            false
        }
    }

    pub fn is_clut_dirty(&mut self, addr: Address, data: &[u8]) -> bool {
        let new_hash = twox_hash::XxHash3_64::oneshot(data);
        let Some(old_hash) = self.tex_cache.get(&addr) else {
            self.tex_cache.insert(addr, new_hash);
            return false;
        };

        if *old_hash == new_hash {
            true
        } else {
            self.tex_cache.insert(addr, new_hash);
            false
        }
    }
}

fn decode_texture(data: &[u8], width: u32, height: u32, format: Format) -> TextureData {
    use gxtex::{
        AlphaChannel, CI4, CI8, CI14X2, Cmpr, FastLuma, FastRgb565, I4, I8, IA4, IA8, Rgb5A3,
        Rgba8, decode,
    };

    let width = width as usize;
    let height = height as usize;

    match format {
        Format::I4 => TextureData::Direct(decode::<I4<FastLuma>>(width, height, data)),
        Format::IA4 => {
            TextureData::Direct(decode::<IA4<FastLuma, AlphaChannel>>(width, height, data))
        }
        Format::I8 => TextureData::Direct(decode::<I8<FastLuma>>(width, height, data)),
        Format::IA8 => {
            TextureData::Direct(decode::<IA8<FastLuma, AlphaChannel>>(width, height, data))
        }
        Format::Rgb565 => TextureData::Direct(decode::<FastRgb565>(width, height, data)),
        Format::Rgb5A3 => TextureData::Direct(decode::<Rgb5A3>(width, height, data)),
        Format::Rgba8 => TextureData::Direct(decode::<Rgba8>(width, height, data)),
        Format::Cmp => TextureData::Direct(decode::<Cmpr>(width, height, data)),
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

pub fn update_texture(sys: &mut System, index: usize) {
    let map = sys.gpu.tex.maps[index];
    let texture_id = render::TextureId(map.address.value());
    let clut_addr = render::ClutAddress(map.clut.tmem_offset().value());
    let clut_fmt = map.clut.format();

    let base = map.address;
    let (len, lod_count) = if map.sampler.min_filter().uses_lods() {
        (
            map.encoding.length_mipmap() as usize,
            map.encoding.lod_count() as usize,
        )
    } else {
        (map.encoding.length() as usize, 1)
    };

    let data = &sys.mem.ram()[base.value() as usize..][..len];
    if !sys.gpu.tex.is_tex_dirty(base, data) {
        let mut current_data = data;
        let mut current_width = map.encoding.width();
        let mut current_height = map.encoding.height();

        let mut mipmap = if map.encoding.format().is_direct() {
            MipmapData::Direct(Vec::with_capacity(lod_count))
        } else {
            MipmapData::Indirect(Vec::with_capacity(lod_count))
        };

        for _ in 0..lod_count {
            mipmap.push(self::decode_texture(
                current_data,
                current_width,
                current_height,
                map.encoding.format(),
            ));

            let consumed =
                Encoding::length_for(current_width, current_height, map.encoding.format()) as usize;

            current_data = &current_data[consumed..];
            current_width /= 2;
            current_height /= 2;
        }

        sys.modules.render.exec(render::Action::LoadTexture {
            id: texture_id,
            texture: render::Texture {
                width: map.encoding.width(),
                height: map.encoding.height(),
                data: mipmap,
            },
        });
    }

    sys.modules.render.exec(render::Action::SetTextureSlot {
        slot: index,
        texture_id,
        sampler: map.sampler,
        scaling: map.scaling,
        clut_addr,
        clut_fmt,
    });
}

pub fn update_clut(sys: &mut System) {
    let load = sys.gpu.tex.clut_load;
    let clut_addr = render::ClutAddress(load.tmem_offset().value());

    let base = Address((sys.gpu.tex.clut_base << 5).with_bits(26, 32, 0));
    let len = load.count().value() as usize * 16 * 2;
    let data = &sys.mem.ram()[base.value() as usize..][..len];

    if !sys.gpu.tex.is_clut_dirty(base, data) {
        let clut = data
            .chunks_exact(2)
            .map(|x| u16::from_be_bytes([x[0], x[1]]))
            .collect();

        sys.modules.render.exec(render::Action::LoadClut {
            addr: clut_addr,
            clut: render::Clut(clut),
        });
    }
}
