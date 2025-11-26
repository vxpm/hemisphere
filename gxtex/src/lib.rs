use bitut::BitUtils;
use zerocopy::{FromBytes, Immutable, IntoBytes};

/// A single RGBA8 pixel.
#[derive(Debug, Clone, Copy, Default, Immutable, IntoBytes, FromBytes)]
#[repr(C)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Pixel {
    pub fn from_rgb565(value: u16) -> Self {
        Self {
            r: value.bits(11, 16) as u8 * 8,
            g: value.bits(5, 11) as u8 * 4,
            b: value.bits(0, 5) as u8 * 8,
            a: 255,
        }
    }

    pub fn from_rgb5a3(value: u16) -> Self {
        if value.bit(15) {
            Pixel {
                r: value.bits(10, 15) as u8 * 8,
                g: value.bits(5, 10) as u8 * 8,
                b: value.bits(0, 5) as u8 * 8,
                a: 255,
            }
        } else {
            Pixel {
                r: value.bits(8, 12) as u8 * 16,
                g: value.bits(4, 8) as u8 * 16,
                b: value.bits(0, 4) as u8 * 16,
                a: value.bits(12, 15) as u8 * 32,
            }
        }
    }

    pub fn lerp(self, rhs: Self, t: f32) -> Self {
        let lerp = |a, b, t| a * (1.0 - t) + b * t;
        Self {
            r: lerp(self.r as f32, rhs.r as f32, t).round() as u8,
            g: lerp(self.g as f32, rhs.g as f32, t).round() as u8,
            b: lerp(self.b as f32, rhs.b as f32, t).round() as u8,
            a: lerp(self.a as f32, rhs.a as f32, t).round() as u8,
        }
    }

    pub fn y(&self) -> u8 {
        let (r, g, b) = (self.r as f32, self.g as f32, self.b as f32);
        (0.257 * r + 0.504 * g + 0.098 * b + 16.0) as u8
    }
}

pub trait Format {
    const NIBBLES_PER_TEXEL: usize;
    const TILE_WIDTH: usize;
    const TILE_HEIGHT: usize;
    const BYTES_PER_TILE: usize = 32;

    type EncodeSettings;

    fn encode_tile(
        settings: &Self::EncodeSettings,
        data: &mut [u8],
        get: impl Fn(usize, usize) -> Pixel,
    );
    fn decode_tile(data: &[u8], set: impl FnMut(usize, usize, Pixel));
}

pub fn encode<F: Format>(
    settings: &F::EncodeSettings,
    stride: usize,
    height: usize,
    data: &[Pixel],
    buffer: &mut [u8],
) {
    let width = data.len() / height;
    assert!(data.len().is_multiple_of(height));
    assert!(stride.is_multiple_of(F::TILE_WIDTH));
    assert!(buffer.len() >= ((width * height * F::NIBBLES_PER_TEXEL).div_ceil(2)));

    let stride_in_tiles = stride / F::TILE_WIDTH;
    let width_in_tiles = width.div_ceil(F::TILE_WIDTH);
    let height_in_tiles = height.div_ceil(F::TILE_HEIGHT);

    for tile_y in 0..height_in_tiles {
        for tile_x in 0..width_in_tiles {
            // where should data be written to?
            let tile_index = tile_y * stride_in_tiles + tile_x;
            let tile_offset = tile_index * F::BYTES_PER_TILE;
            let out = &mut buffer[tile_offset..][..F::BYTES_PER_TILE];

            // find pixels in this tile
            let base_x = tile_x * F::TILE_WIDTH;
            let base_y = tile_y * F::TILE_HEIGHT;
            F::encode_tile(&settings, out, |x, y| {
                assert!(x <= F::TILE_WIDTH);
                assert!(y <= F::TILE_HEIGHT);
                let image_index = (base_y + y) * width + (base_x + x);
                data[image_index]
            });
        }
    }
}

pub fn decode<F: Format>(width: usize, height: usize, data: &[u8]) -> Vec<Pixel> {
    let mut pixels = vec![Pixel::default(); width as usize * height as usize];

    let width_in_tiles = width.div_ceil(F::TILE_WIDTH);
    let height_in_tiles = height.div_ceil(F::TILE_HEIGHT);

    let full_width = width_in_tiles * F::TILE_WIDTH;
    let full_height = height_in_tiles * F::TILE_HEIGHT;
    assert!(data.len() >= ((full_width * full_height * F::NIBBLES_PER_TEXEL).div_ceil(2)));

    for tile_y in 0..height_in_tiles {
        for tile_x in 0..width_in_tiles {
            let tile_index = tile_y * width_in_tiles + tile_x;
            let tile_offset = tile_index * F::BYTES_PER_TILE;
            let tile_data = &data[tile_offset..][..F::BYTES_PER_TILE];

            let base_x = tile_x * F::TILE_WIDTH;
            let base_y = tile_y * F::TILE_HEIGHT;
            F::decode_tile(tile_data, |x, y, value| {
                assert!(x <= F::TILE_WIDTH);
                assert!(y <= F::TILE_HEIGHT);
                let image_index = (base_y + y) * width + (base_x + x);
                if let Some(pixel) = pixels.get_mut(image_index) {
                    *pixel = value;
                }
            });
        }
    }

    pixels
}

#[inline(always)]
fn range_conv(value: u8, old_max: u8, new_max: u8) -> u8 {
    ((value as f32 / old_max as f32) * new_max as f32) as u8
}

pub enum IntensitySource {
    Y,
    R,
    G,
    B,
}

pub struct Intensity4;

impl Format for Intensity4 {
    const NIBBLES_PER_TEXEL: usize = 1;
    const TILE_WIDTH: usize = 8;
    const TILE_HEIGHT: usize = 8;

    type EncodeSettings = ();

    fn encode_tile(_: &Self::EncodeSettings, data: &mut [u8], get: impl Fn(usize, usize) -> Pixel) {
        for y in 0..Self::TILE_HEIGHT {
            for x in 0..Self::TILE_WIDTH {
                let pixel = get(x, y);
                let intensity = range_conv(pixel.y(), 255, 15);

                let index = y * Self::TILE_WIDTH + x;
                let current = data[index / 2];

                let new = if index % 2 == 0 {
                    current.with_bits(4, 8, intensity)
                } else {
                    current.with_bits(0, 4, intensity)
                };

                data[index / 2] = new;
            }
        }
    }

    fn decode_tile(data: &[u8], mut set: impl FnMut(usize, usize, Pixel)) {
        for y in 0..Self::TILE_HEIGHT {
            for x in 0..Self::TILE_WIDTH {
                let index = y * Self::TILE_WIDTH + x;
                let value = data[index / 2];
                let intensity = range_conv(
                    if index % 2 == 0 {
                        value.bits(4, 8)
                    } else {
                        value.bits(0, 4)
                    },
                    15,
                    255,
                );

                set(
                    x,
                    y,
                    Pixel {
                        r: intensity,
                        g: intensity,
                        b: intensity,
                        a: intensity,
                    },
                )
            }
        }
    }
}

pub struct Intensity4Alpha;

impl Format for Intensity4Alpha {
    const NIBBLES_PER_TEXEL: usize = 2;
    const TILE_WIDTH: usize = 8;
    const TILE_HEIGHT: usize = 4;

    type EncodeSettings = ();

    fn encode_tile(_: &Self::EncodeSettings, data: &mut [u8], get: impl Fn(usize, usize) -> Pixel) {
        for y in 0..Self::TILE_HEIGHT {
            for x in 0..Self::TILE_WIDTH {
                let pixel = get(x, y);
                let intensity = range_conv(pixel.y(), 255, 15);
                let alpha = range_conv(pixel.a, 255, 15);

                let index = y * Self::TILE_WIDTH + x;
                data[index] = 0.with_bits(0, 4, intensity).with_bits(4, 8, alpha);
            }
        }
    }

    fn decode_tile(data: &[u8], mut set: impl FnMut(usize, usize, Pixel)) {
        for y in 0..Self::TILE_HEIGHT {
            for x in 0..Self::TILE_WIDTH {
                let index = y * Self::TILE_WIDTH + x;
                let value = data[index];
                let intensity = range_conv(value.bits(0, 4), 15, 255);
                let alpha = range_conv(value.bits(4, 8), 15, 255);

                set(
                    x,
                    y,
                    Pixel {
                        r: intensity,
                        g: intensity,
                        b: intensity,
                        a: alpha,
                    },
                )
            }
        }
    }
}

pub struct Intensity8;

impl Format for Intensity8 {
    const NIBBLES_PER_TEXEL: usize = 2;
    const TILE_WIDTH: usize = 8;
    const TILE_HEIGHT: usize = 4;

    type EncodeSettings = ();

    fn encode_tile(
        settings: &Self::EncodeSettings,
        data: &mut [u8],
        get: impl Fn(usize, usize) -> Pixel,
    ) {
        todo!()
    }

    fn decode_tile(data: &[u8], mut set: impl FnMut(usize, usize, Pixel)) {
        for y in 0..Self::TILE_HEIGHT {
            for x in 0..Self::TILE_WIDTH {
                let index = y * Self::TILE_WIDTH + x;
                let intensity = data[index];

                set(
                    x,
                    y,
                    Pixel {
                        r: intensity,
                        g: intensity,
                        b: intensity,
                        a: intensity,
                    },
                )
            }
        }
    }
}

pub struct Intensity8Alpha;

impl Format for Intensity8Alpha {
    const NIBBLES_PER_TEXEL: usize = 4;
    const TILE_WIDTH: usize = 4;
    const TILE_HEIGHT: usize = 4;

    type EncodeSettings = ();

    fn encode_tile(
        settings: &Self::EncodeSettings,
        data: &mut [u8],
        get: impl Fn(usize, usize) -> Pixel,
    ) {
        todo!()
    }

    fn decode_tile(data: &[u8], mut set: impl FnMut(usize, usize, Pixel)) {
        for y in 0..Self::TILE_HEIGHT {
            for x in 0..Self::TILE_WIDTH {
                let index = y * Self::TILE_WIDTH + x;
                let alpha = data[2 * index];
                let intensity = data[2 * index + 1];

                set(
                    x,
                    y,
                    Pixel {
                        r: intensity,
                        g: intensity,
                        b: intensity,
                        a: alpha,
                    },
                )
            }
        }
    }
}

pub struct Rgb565;

impl Format for Rgb565 {
    const NIBBLES_PER_TEXEL: usize = 4;
    const TILE_WIDTH: usize = 4;
    const TILE_HEIGHT: usize = 4;

    type EncodeSettings = ();

    fn encode_tile(
        settings: &Self::EncodeSettings,
        data: &mut [u8],
        get: impl Fn(usize, usize) -> Pixel,
    ) {
        todo!()
    }

    fn decode_tile(data: &[u8], mut set: impl FnMut(usize, usize, Pixel)) {
        for y in 0..Self::TILE_HEIGHT {
            for x in 0..Self::TILE_WIDTH {
                let index = y * Self::TILE_WIDTH + x;
                let value = u16::from_be_bytes([data[2 * index], data[2 * index + 1]]);
                set(x, y, Pixel::from_rgb565(value))
            }
        }
    }
}

pub struct Rgb5A3;

impl Format for Rgb5A3 {
    const NIBBLES_PER_TEXEL: usize = 4;
    const TILE_WIDTH: usize = 4;
    const TILE_HEIGHT: usize = 4;

    type EncodeSettings = ();

    fn encode_tile(
        settings: &Self::EncodeSettings,
        data: &mut [u8],
        get: impl Fn(usize, usize) -> Pixel,
    ) {
        todo!()
    }

    fn decode_tile(data: &[u8], mut set: impl FnMut(usize, usize, Pixel)) {
        for y in 0..Self::TILE_HEIGHT {
            for x in 0..Self::TILE_WIDTH {
                let index = y * Self::TILE_WIDTH + x;
                let value = u16::from_be_bytes([data[2 * index], data[2 * index + 1]]);
                set(x, y, Pixel::from_rgb5a3(value))
            }
        }
    }
}

pub struct Rgba8;

impl Format for Rgba8 {
    const NIBBLES_PER_TEXEL: usize = 4;
    const TILE_WIDTH: usize = 4;
    const TILE_HEIGHT: usize = 4;
    const BYTES_PER_TILE: usize = 64;

    type EncodeSettings = ();

    fn encode_tile(
        settings: &Self::EncodeSettings,
        data: &mut [u8],
        get: impl Fn(usize, usize) -> Pixel,
    ) {
        todo!()
    }

    fn decode_tile(data: &[u8], mut set: impl FnMut(usize, usize, Pixel)) {
        for y in 0..Self::TILE_HEIGHT {
            for x in 0..Self::TILE_WIDTH {
                let index = y * Self::TILE_WIDTH + x;
                let offset = 2 * index;

                let ar_index = offset;
                let gb_index = 32 + offset;

                let (a, r) = (data[ar_index], data[ar_index + 1]);
                let (g, b) = (data[gb_index], data[gb_index + 1]);

                set(x, y, Pixel { r, g, b, a })
            }
        }
    }
}

pub struct Cmpr;

impl Format for Cmpr {
    const NIBBLES_PER_TEXEL: usize = 1;
    const TILE_WIDTH: usize = 8;
    const TILE_HEIGHT: usize = 8;

    type EncodeSettings = ();

    fn encode_tile(
        settings: &Self::EncodeSettings,
        data: &mut [u8],
        get: impl Fn(usize, usize) -> Pixel,
    ) {
        todo!()
    }

    fn decode_tile(data: &[u8], mut set: impl FnMut(usize, usize, Pixel)) {
        for sub_y in 0..2 {
            for sub_x in 0..2 {
                let sub_base_x = sub_x * 4;
                let sub_base_y = sub_y * 4;
                let sub_base_index = sub_y * 2 + sub_x;
                let sub_offset = 8 * sub_base_index;

                // read palette (first 4 bytes)
                let a = u16::from_be_bytes([data[sub_offset], data[sub_offset + 1]]);
                let b = u16::from_be_bytes([data[sub_offset + 2], data[sub_offset + 3]]);

                let mut palette = [Pixel::default(); 4];
                palette[0] = Pixel::from_rgb565(a);
                palette[1] = Pixel::from_rgb565(b);

                if a > b {
                    palette[2] = palette[0].lerp(palette[1], 1.0 / 3.0);
                    palette[3] = palette[0].lerp(palette[1], 2.0 / 3.0);
                } else {
                    palette[2] = palette[0].lerp(palette[1], 0.5);
                }

                // read pixels (last 4 bytes)
                let mut indices = data[sub_offset + 4..][..4]
                    .iter()
                    .copied()
                    .flat_map(|b| [b.bits(6, 8), b.bits(4, 6), b.bits(2, 4), b.bits(0, 2)]);

                for inner_y in 0..4 {
                    for inner_x in 0..4 {
                        let index = indices.next().unwrap();
                        let pixel = palette[index as usize];

                        let x = sub_base_x + inner_x;
                        let y = sub_base_y + inner_y;
                        set(x, y, pixel);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{Format, Intensity4, Intensity4Alpha, Pixel, decode, encode};

    fn test_format<F: Format>(settings: &F::EncodeSettings, name: &str) {
        let img = image::open("resources/test.webp").unwrap();
        let pixels = img
            .to_rgba8()
            .pixels()
            .map(|p| Pixel {
                r: p.0[0],
                g: p.0[1],
                b: p.0[2],
                a: p.0[3],
            })
            .collect::<Vec<_>>();

        let mut encoded = vec![0; img.width() as usize * img.height() as usize];
        encode::<F>(
            settings,
            img.width() as usize,
            img.height() as usize,
            &pixels,
            &mut encoded,
        );

        let decoded = decode::<F>(img.width() as usize, img.height() as usize, &encoded);
        let img = image::RgbaImage::from_vec(
            img.width(),
            img.height(),
            decoded
                .into_iter()
                .flat_map(|p| [p.r, p.g, p.b, p.a])
                .collect(),
        )
        .unwrap();

        _ = std::fs::create_dir("local");
        img.save(format!("local/test_out_{name}.png")).unwrap();
    }

    #[test]
    fn test() {
        test_format::<Intensity4>(&(), "I4");
        test_format::<Intensity4Alpha>(&(), "I4A");
    }
}
