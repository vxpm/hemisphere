use bitos::{BitUtils, bitos};
use ordered_float::OrderedFloat;
use zerocopy::{FromBytes, Immutable, IntoBytes};

#[derive(Debug, Clone, Copy, Default, Immutable, IntoBytes, FromBytes)]
#[repr(C)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub fn lerp(self, rhs: Self, t: f32) -> Self {
        let lerp = |a, b, t| a * (1.0 - t) + b * t;
        Self {
            r: lerp(self.r as f32, rhs.r as f32, t).round() as u8,
            g: lerp(self.g as f32, rhs.g as f32, t).round() as u8,
            b: lerp(self.b as f32, rhs.b as f32, t).round() as u8,
            a: lerp(self.a as f32, rhs.a as f32, t).round() as u8,
        }
    }

    pub fn from_rgb565(value: u16) -> Self {
        Rgba8 {
            r: value.bits(11, 16) as u8 * 8,
            g: value.bits(5, 11) as u8 * 4,
            b: value.bits(0, 5) as u8 * 8,
            a: 255,
        }
    }
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Abgr8 {
    #[bits(0..8)]
    pub a: u8,
    #[bits(8..16)]
    pub b: u8,
    #[bits(16..24)]
    pub g: u8,
    #[bits(24..32)]
    pub r: u8,
}

#[derive(Debug, Clone, Copy, Immutable, IntoBytes, Default)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
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

impl From<Abgr8> for Rgba {
    fn from(value: Abgr8) -> Self {
        Self {
            r: value.r() as f32 / 255.0,
            g: value.g() as f32 / 255.0,
            b: value.b() as f32 / 255.0,
            a: value.a() as f32 / 255.0,
        }
    }
}
