use bitos::{
    BitUtils, bitos,
    integer::{u2, u10},
};
use common::Address;
use common::Primitive;
use zerocopy::{Immutable, IntoBytes};

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
        let pixels = self.width() * self.height();
        match self.data_format() {
            DataFormat::Intensity8 => pixels,
            DataFormat::Rgb565 => pixels * 2,
            DataFormat::Rgb5A3 => pixels * 2,
            DataFormat::Rgba8 => pixels * 4,
            _ => todo!("format {:?}", self.data_format()),
        }
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

#[derive(Debug, Clone, Copy, Immutable, IntoBytes)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

fn decode_basic_tex<
    const TILE_WIDTH: u32,
    const TILE_HEIGHT: u32,
    F: FnMut(&[u8], usize) -> Rgba8,
>(
    data: &[u8],
    width: u32,
    height: u32,
    mut decode: F,
) -> Vec<Rgba8> {
    let mut pixels = vec![
        Rgba8 {
            r: 0,
            g: 0,
            b: 0,
            a: 0
        };
        width as usize * height as usize
    ];

    let width_in_tiles = width.div_ceil(TILE_WIDTH);
    let height_in_tiles = height.div_ceil(TILE_HEIGHT);

    let mut data_index = 0;
    for tile_y in 0..height_in_tiles {
        for tile_x in 0..width_in_tiles {
            for inner_y in 0..TILE_HEIGHT {
                for inner_x in 0..TILE_WIDTH {
                    let x = tile_x * TILE_WIDTH + inner_x;
                    let y = tile_y * TILE_HEIGHT + inner_y;
                    let image_index = y * width + x;

                    if let Some(pixel) = pixels.get_mut(image_index as usize) {
                        *pixel = decode(data, data_index);
                    }

                    data_index += 1;
                }
            }
        }
    }

    pixels
}

pub fn decode_texture(data: &[u8], format: Format) -> Vec<Rgba8> {
    match format.data_format() {
        DataFormat::Intensity8 => {
            decode_basic_tex::<8, 4, _>(data, format.width(), format.height(), |data, index| {
                let intensity = data[index];
                Rgba8 {
                    r: intensity,
                    g: intensity,
                    b: intensity,
                    a: 255,
                }
            })
        }
        DataFormat::Rgb565 => {
            decode_basic_tex::<4, 4, _>(data, format.width(), format.height(), |data, index| {
                let pixel = u16::read_be_bytes(&data[2 * index..]);
                Rgba8 {
                    r: pixel.bits(11, 16) as u8 * 8,
                    g: pixel.bits(5, 11) as u8 * 4,
                    b: pixel.bits(0, 5) as u8 * 8,
                    a: 255,
                }
            })
        }
        DataFormat::Rgb5A3 => {
            decode_basic_tex::<4, 4, _>(data, format.width(), format.height(), |data, index| {
                let pixel = u16::read_be_bytes(&data[2 * index..]);
                if pixel.bit(15) {
                    Rgba8 {
                        r: pixel.bits(10, 15) as u8 * 8,
                        g: pixel.bits(5, 10) as u8 * 8,
                        b: pixel.bits(0, 5) as u8 * 8,
                        a: 255,
                    }
                } else {
                    Rgba8 {
                        r: pixel.bits(8, 12) as u8 * 16,
                        g: pixel.bits(4, 8) as u8 * 16,
                        b: pixel.bits(0, 4) as u8 * 16,
                        a: pixel.bits(12, 15) as u8 * 32,
                    }
                }
            })
        }
        DataFormat::Rgba8 => {
            decode_basic_tex::<4, 4, _>(data, format.width(), format.height(), |data, index| {
                let block = index / 16;
                let pixel = index % 16;

                let [a, r] = u16::read_be_bytes(&data[64 * block + 2 * pixel..]).to_be_bytes();
                let [g, b] = u16::read_be_bytes(&data[64 * block + 2 * pixel + 32..]).to_be_bytes();

                Rgba8 { r, g, b, a }
            })
        }
        _ => todo!("format {format:?}"),
    }
}
