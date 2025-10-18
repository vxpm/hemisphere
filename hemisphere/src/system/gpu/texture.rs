use bitos::{
    bitos,
    integer::{u2, u10},
};
use common::Address;

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
    Indexed4 = 0x0,
    Indexed8 = 0x1,
    Indexed4Alpha = 0x2,
    Indexed8Alpha = 0x3,
    Rgb565 = 0x4,
    Rgb4A3 = 0x5,
    Rgba8 = 0x6,
    // everything below is a mystery
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

impl DataFormat {
    pub fn size(&self) -> u32 {
        match self {
            Self::Rgb565 => 2,
            _ => todo!(),
        }
    }
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

    pub fn size(&self) -> u32 {
        self.width() * self.height() * self.data_format().size()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TextureMap {
    pub address: Address,
    pub format: Format,
    pub mode: Mode,
    pub dirty: bool,
}

impl TextureMap {}

#[derive(Debug, Default)]
pub struct Interface {
    pub maps: [TextureMap; 8],
}

impl Interface {}
