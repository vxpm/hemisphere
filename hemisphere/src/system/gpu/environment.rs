use bitos::{
    bitos,
    integer::{u2, u3},
};

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChannel {
    Color0 = 0x0,
    Color1 = 0x1,
    Alpha0 = 0x2,
    Alpha1 = 0x3,
    ColorAlpha0 = 0x4,
    ColorAlpha1 = 0x5,
    Zero = 0x6,
    AlphaBump = 0x7,
}

#[bitos(10)]
#[derive(Debug, Default)]
pub struct StageOrder {
    #[bits(0..3)]
    pub map: u3,
    #[bits(3..6)]
    pub coord: u3,
    #[bits(6)]
    pub map_enable: bool,
    #[bits(7..10)]
    pub color: ColorChannel,
}

#[bitos(32)]
#[derive(Debug, Default)]
pub struct OrderPair {
    #[bits(0..10)]
    pub a: StageOrder,
    #[bits(12..22)]
    pub b: StageOrder,
}

#[bitos(4)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorInputSrc {
    R3Color = 0x0,
    R3Alpha = 0x1,
    R0Color = 0x2,
    R0Alpha = 0x3,
    R1Color = 0x4,
    R1Alpha = 0x5,
    R2Color = 0x6,
    R2Alpha = 0x7,
    TexColor = 0x8,
    TexAlpha = 0x9,
    RasterColor = 0xA,
    RasterAlpha = 0xB,
    One = 0xC,
    Half = 0xD,
    Constant = 0xE,
    Zero = 0xF,
}

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaInputSrc {
    R3Alpha = 0x0,
    R0Alpha = 0x1,
    R1Alpha = 0x2,
    R2Alpha = 0x3,
    TexAlpha = 0x4,
    RasterAlpha = 0x5,
    Constant = 0x6,
    Zero = 0x7,
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bias {
    Zero = 0b00,
    PositiveHalf = 0b01,
    NegativeHalf = 0b10,
    Reserved = 0b11,
}

impl Bias {
    pub fn value(self) -> f32 {
        match self {
            Self::Zero => 0.0,
            Self::PositiveHalf => 0.5,
            Self::NegativeHalf => -0.5,
            Self::Reserved => 1.0,
        }
    }
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    One = 0b00,
    Two = 0b01,
    Four = 0b10,
    Half = 0b11,
}

impl Scale {
    pub fn value(self) -> f32 {
        match self {
            Self::One => 1.0,
            Self::Two => 2.0,
            Self::Four => 4.0,
            Self::Half => 0.5,
        }
    }
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputDst {
    R3 = 0b00,
    R0 = 0b01,
    R1 = 0b10,
    R2 = 0b11,
}

#[bitos(32)]
#[derive(Debug, Clone, Default)]
pub struct StageColor {
    #[bits(0..4)]
    pub input_d: ColorInputSrc,
    #[bits(4..8)]
    pub input_c: ColorInputSrc,
    #[bits(8..12)]
    pub input_b: ColorInputSrc,
    #[bits(12..16)]
    pub input_a: ColorInputSrc,
    #[bits(16..18)]
    pub bias: Bias,
    #[bits(18)]
    pub negate: bool,
    #[bits(19)]
    pub clamp: bool,
    #[bits(20..22)]
    pub scale: Scale,
    #[bits(22..24)]
    pub output: OutputDst,
}

#[bitos(32)]
#[derive(Debug, Clone, Default)]
pub struct StageAlpha {
    #[bits(0..2)]
    pub rasterizer_swap: u2,
    #[bits(2..4)]
    pub texture_swap: u2,
    #[bits(4..7)]
    pub input_d: AlphaInputSrc,
    #[bits(7..10)]
    pub input_c: AlphaInputSrc,
    #[bits(10..13)]
    pub input_b: AlphaInputSrc,
    #[bits(13..16)]
    pub input_a: AlphaInputSrc,
    #[bits(16..18)]
    pub bias: Bias,
    #[bits(18)]
    pub negate: bool,
    #[bits(19)]
    pub clamp: bool,
    #[bits(20..22)]
    pub scale: Scale,
    #[bits(22..24)]
    pub output: OutputDst,
}

#[derive(Debug, Default)]
pub struct Interface {
    pub stages: u8,
    pub channels: u8,
    pub order_pairs: [OrderPair; 8],
    pub color_stages: [StageColor; 16],
    pub alpha_stages: [StageAlpha; 16],
}
