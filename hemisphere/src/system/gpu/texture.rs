use crate::Primitive;
use bitos::{
    BitUtils, bitos,
    integer::{u2, u10},
};
use gekko::Address;
use rustc_hash::FxBuildHasher;
use std::collections::HashMap;
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

#[derive(Debug, Clone, Copy, Default, Immutable, IntoBytes)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    fn lerp(self, rhs: Self, t: f32) -> Self {
        let lerp = |a, b, t| a * (1.0 - t) + b * t;
        Self {
            r: lerp(self.r as f32, rhs.r as f32, t).round() as u8,
            g: lerp(self.g as f32, rhs.g as f32, t).round() as u8,
            b: lerp(self.b as f32, rhs.b as f32, t).round() as u8,
            a: lerp(self.a as f32, rhs.a as f32, t).round() as u8,
        }
    }

    fn rgb565(value: u16) -> Self {
        Rgba8 {
            r: value.bits(11, 16) as u8 * 8,
            g: value.bits(5, 11) as u8 * 4,
            b: value.bits(0, 5) as u8 * 8,
            a: 255,
        }
    }
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

fn decode_cmpr_tex(data: &[u8], width: u32, height: u32) -> Vec<Rgba8> {
    const TILE_WIDTH: u32 = 8;
    const TILE_HEIGHT: u32 = 8;

    let mut pixels = vec![Default::default(); width as usize * height as usize];

    let width_in_tiles = width.div_ceil(TILE_WIDTH);
    let height_in_tiles = height.div_ceil(TILE_HEIGHT);

    let mut data_index = 0;
    for tile_y in 0..height_in_tiles {
        for tile_x in 0..width_in_tiles {
            let base_tile_x = tile_x * TILE_WIDTH;
            let base_tile_y = tile_y * TILE_HEIGHT;

            for subtile_y in 0..2 {
                for subtile_x in 0..2 {
                    let subtile_base_x = base_tile_x + subtile_x * 4;
                    let subtile_base_y = base_tile_y + subtile_y * 4;

                    // read palette
                    let a = u16::read_be_bytes(&data[data_index..]);
                    let b = u16::read_be_bytes(&data[data_index + 2..]);

                    let mut palette = [Rgba8::default(); 4];
                    palette[0] = Rgba8::rgb565(a);
                    palette[1] = Rgba8::rgb565(b);

                    if a > b {
                        palette[2] = palette[0].lerp(palette[1], 1.0 / 3.0);
                        palette[3] = palette[0].lerp(palette[1], 2.0 / 3.0);
                    } else {
                        palette[2] = palette[0].lerp(palette[1], 0.5);
                    }

                    let mut indices = data[data_index + 4..][..4]
                        .iter()
                        .copied()
                        .flat_map(|b| [b.bits(6, 8), b.bits(4, 6), b.bits(2, 4), b.bits(0, 2)]);

                    for inner_y in 0..4 {
                        for inner_x in 0..4 {
                            let index = indices.next().unwrap();
                            let color = palette[index as usize];

                            let x = subtile_base_x + inner_x;
                            let y = subtile_base_y + inner_y;
                            let image_index = y * width + x;

                            if let Some(pixel) = pixels.get_mut(image_index as usize) {
                                *pixel = color;
                            }
                        }
                    }

                    data_index += 8;
                }
            }
        }
    }

    pixels
}

pub fn decode_texture(data: &[u8], format: Format) -> Vec<Rgba8> {
    match format.data_format() {
        DataFormat::Intensity4 => {
            decode_basic_tex::<8, 8, _>(data, format.width(), format.height(), |data, index| {
                let value = data[index / 2];
                let intensity = if index % 2 == 0 {
                    value.bits(4, 8)
                } else {
                    value.bits(0, 4)
                } * 16;

                Rgba8 {
                    r: intensity,
                    g: intensity,
                    b: intensity,
                    a: intensity,
                }
            })
        }
        DataFormat::Intensity4Alpha => {
            decode_basic_tex::<8, 8, _>(data, format.width(), format.height(), |data, index| {
                let value = data[index];
                let intensity = value.bits(0, 4) * 16;
                let alpha = value.bits(4, 8) * 16;

                Rgba8 {
                    r: intensity,
                    g: intensity,
                    b: intensity,
                    a: alpha,
                }
            })
        }
        DataFormat::Intensity8 => {
            decode_basic_tex::<8, 4, _>(data, format.width(), format.height(), |data, index| {
                let intensity = data[index];
                Rgba8 {
                    r: intensity,
                    g: intensity,
                    b: intensity,
                    a: intensity,
                }
            })
        }
        DataFormat::Intensity8Alpha => {
            decode_basic_tex::<4, 4, _>(data, format.width(), format.height(), |data, index| {
                let [alpha, intensity] = u16::read_be_bytes(&data[2 * index..]).to_be_bytes();
                Rgba8 {
                    r: intensity,
                    g: intensity,
                    b: intensity,
                    a: alpha,
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
        DataFormat::Cmp => decode_cmpr_tex(data, format.width(), format.height()),
        DataFormat::C8 | DataFormat::C4 => {
            vec![
                Rgba8 {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255
                };
                (format.width() * format.height()) as usize
            ]
        }
        _ => todo!("format {format:?}"),
    }
}
