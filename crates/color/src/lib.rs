use bitut::BitUtils;
use ordered_float::OrderedFloat;
use zerocopy::{FromBytes, Immutable, IntoBytes};

/// Converts a value in range `0..=OLD_MAX` to a value in the range `0..=NEW_MAX`.
#[inline(always)]
pub fn convert_range<const OLD_MAX: u32, const NEW_MAX: u32>(value: u8) -> u8 {
    const {
        assert!(OLD_MAX != 0);
        assert!(OLD_MAX <= 255);
        assert!(NEW_MAX <= 255);
    };

    let value = value as u32;
    ((value * NEW_MAX + OLD_MAX / 2) / OLD_MAX) as u8
}

#[inline(always)]
fn fast_range_conv_31_to_255(value: u8) -> u8 {
    // 255 / 31 is approx 8.25, so multiply value by 8 and divide by 4 then add them
    (value << 3) | (value >> 2)
}

#[inline(always)]
fn fast_range_conv_63_to_255(value: u8) -> u8 {
    // 255 / 63 is approx 4.0625, so multiply value by 4 and divide by 16 then add them
    (value << 2) | (value >> 4)
}

#[inline(always)]
fn fast_range_conv_255_to_31(value: u8) -> u8 {
    // 31 / 255 is approx 0.125, so divide by 8
    value >> 3
}

#[inline(always)]
fn fast_range_conv_255_to_63(value: u8) -> u8 {
    // 63 / 255 is approx 0.25, so divide by 4
    value >> 2
}

/// A single RGBA8 pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Immutable, IntoBytes, FromBytes, Default)]
#[repr(C)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    #[inline(always)]
    pub fn from_ia8(value: u16) -> Self {
        let [alpha, intensity] = value.to_le_bytes();
        Self {
            r: intensity,
            g: intensity,
            b: intensity,
            a: alpha,
        }
    }

    #[inline(always)]
    pub fn from_rgb565(value: u16) -> Self {
        Self {
            r: convert_range::<31, 255>(value.bits(11, 16) as u8),
            g: convert_range::<63, 255>(value.bits(5, 11) as u8),
            b: convert_range::<31, 255>(value.bits(0, 5) as u8),
            a: 255,
        }
    }

    #[inline(always)]
    pub fn from_rgb565_fast(value: u16) -> Self {
        Self {
            r: fast_range_conv_31_to_255(value.bits(11, 16) as u8),
            g: fast_range_conv_63_to_255(value.bits(5, 11) as u8),
            b: fast_range_conv_31_to_255(value.bits(0, 5) as u8),
            a: 255,
        }
    }

    #[inline(always)]
    pub fn to_rgb565(self) -> u16 {
        let r = convert_range::<255, 31>(self.r);
        let g = convert_range::<255, 63>(self.g);
        let b = convert_range::<255, 31>(self.b);
        0u16.with_bits(0, 5, b as u16)
            .with_bits(5, 11, g as u16)
            .with_bits(11, 16, r as u16)
    }

    #[inline(always)]
    pub fn to_rgb565_fast(self) -> u16 {
        let r = fast_range_conv_255_to_31(self.r);
        let g = fast_range_conv_255_to_63(self.g);
        let b = fast_range_conv_255_to_31(self.b);
        0u16.with_bits(0, 5, b as u16)
            .with_bits(5, 11, g as u16)
            .with_bits(11, 16, r as u16)
    }

    #[inline(always)]
    pub fn from_rgb5a3(value: u16) -> Self {
        if value.bit(15) {
            Rgba8 {
                r: convert_range::<31, 255>(value.bits(10, 15) as u8),
                g: convert_range::<31, 255>(value.bits(5, 10) as u8),
                b: convert_range::<31, 255>(value.bits(0, 5) as u8),
                a: 255,
            }
        } else {
            Rgba8 {
                r: convert_range::<15, 255>(value.bits(8, 12) as u8),
                g: convert_range::<15, 255>(value.bits(4, 8) as u8),
                b: convert_range::<15, 255>(value.bits(0, 4) as u8),
                a: value.bits(12, 15) as u8 * 32,
            }
        }
    }

    #[inline(always)]
    pub fn to_rgb5a3(self) -> u16 {
        if self.a == 255 {
            let r = convert_range::<255, 31>(self.r);
            let g = convert_range::<255, 31>(self.g);
            let b = convert_range::<255, 31>(self.b);
            0u16.with_bits(0, 5, b as u16)
                .with_bits(5, 10, g as u16)
                .with_bits(10, 15, r as u16)
                .with_bit(15, true)
        } else {
            let r = convert_range::<255, 15>(self.r);
            let g = convert_range::<255, 15>(self.g);
            let b = convert_range::<255, 15>(self.b);
            let a = self.a / 32;

            0u16.with_bits(0, 4, b as u16)
                .with_bits(4, 8, g as u16)
                .with_bits(8, 12, r as u16)
                .with_bits(12, 15, a as u16)
                .with_bit(15, false)
        }
    }

    #[inline(always)]
    pub fn lerp(self, rhs: Self, t: f32) -> Self {
        let lerp = |a, b, t| a * (1.0 - t) + b * t;
        Self {
            r: lerp(self.r as f32, rhs.r as f32, t).round() as u8,
            g: lerp(self.g as f32, rhs.g as f32, t).round() as u8,
            b: lerp(self.b as f32, rhs.b as f32, t).round() as u8,
            a: lerp(self.a as f32, rhs.a as f32, t).round() as u8,
        }
    }

    #[inline(always)]
    pub fn y(self) -> u8 {
        let (r, g, b) = (self.r as f32, self.g as f32, self.b as f32);
        (0.257 * r + 0.504 * g + 0.098 * b + 16.0) as u8
    }

    #[inline(always)]
    pub fn fast_y(self) -> u8 {
        let (r, g, b) = (self.r as u16, self.g as u16, self.b as u16);
        (r / 4 + g / 2 + b / 8 + 16).min(255) as u8
    }
}

#[derive(Debug, Clone, Copy, Default, FromBytes, Immutable)]
#[repr(C)]
pub struct Abgr8 {
    pub a: u8,
    pub b: u8,
    pub g: u8,
    pub r: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct Rgba16 {
    pub r: i16,
    pub g: i16,
    pub b: i16,
    pub a: i16,
}

impl std::fmt::Debug for Rgba16 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Rgba::from(*self).fmt(f)
    }
}

#[derive(Clone, Copy, Immutable, IntoBytes, Default)]
#[repr(C)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    #[inline(always)]
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    #[inline(always)]
    pub fn rgb(self) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: 1.0,
        }
    }
}

impl std::fmt::Debug for Rgba {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rgba({}, {}, {}, {})", self.r, self.g, self.b, self.a)
    }
}

impl PartialEq for Rgba {
    fn eq(&self, other: &Self) -> bool {
        OrderedFloat(self.r) == OrderedFloat(other.r)
            && OrderedFloat(self.g) == OrderedFloat(other.g)
            && OrderedFloat(self.b) == OrderedFloat(other.b)
            && OrderedFloat(self.a) == OrderedFloat(other.a)
    }
}

impl Eq for Rgba {}

impl std::hash::Hash for Rgba {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        OrderedFloat(self.r).hash(state);
        OrderedFloat(self.g).hash(state);
        OrderedFloat(self.b).hash(state);
        OrderedFloat(self.a).hash(state);
    }
}

impl From<Rgba8> for Rgba {
    fn from(value: Rgba8) -> Self {
        Self {
            r: value.r as f32 / 255.0,
            g: value.g as f32 / 255.0,
            b: value.b as f32 / 255.0,
            a: value.a as f32 / 255.0,
        }
    }
}

impl From<Abgr8> for Rgba {
    fn from(value: Abgr8) -> Self {
        Self {
            r: value.r as f32 / 255.0,
            g: value.g as f32 / 255.0,
            b: value.b as f32 / 255.0,
            a: value.a as f32 / 255.0,
        }
    }
}

// NOTE: this is correct, RGBA16 is special
impl From<Rgba16> for Rgba {
    fn from(value: Rgba16) -> Self {
        Self {
            r: value.r as f32 / 255.0,
            g: value.g as f32 / 255.0,
            b: value.b as f32 / 255.0,
            a: value.a as f32 / 255.0,
        }
    }
}
