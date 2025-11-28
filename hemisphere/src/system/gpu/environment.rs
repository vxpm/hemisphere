use bitos::{
    bitos,
    integer::{u2, u3},
};

use crate::system::gpu::colors::Rgba;

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChannel {
    Channel0 = 0x0,
    Channel1 = 0x1,
    Reserved0 = 0x2,
    Reserved1 = 0x3,
    Reserved2 = 0x4,
    AlphaBump = 0x5,
    AlphaBumpNormalized = 0x6,
    Zero = 0x7,
}

#[bitos(10)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct StageRefs {
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
pub struct StageRefsPair {
    #[bits(0..10)]
    pub a: StageRefs,
    #[bits(12..22)]
    pub b: StageRefs,
}

#[bitos(5)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Constant {
    One = 0x00,
    SevenEights = 0x01,
    SixEights = 0x02,
    FiveEights = 0x03,
    FourEights = 0x04,
    ThreeEights = 0x05,
    TwoEights = 0x06,
    OneEight = 0x07,
    Reserved0 = 0x08,
    Reserved1 = 0x09,
    Reserved2 = 0x0A,
    Reserved3 = 0x0B,
    Const0 = 0x0C,
    Const1 = 0x0D,
    Const2 = 0x0E,
    Const3 = 0x0F,
    Const0R = 0x10,
    Const1R = 0x11,
    Const2R = 0x12,
    Const3R = 0x13,
    Const0G = 0x14,
    Const1G = 0x15,
    Const2G = 0x16,
    Const3G = 0x17,
    Const0B = 0x18,
    Const1B = 0x19,
    Const2B = 0x1A,
    Const3B = 0x1B,
    Const0A = 0x1C,
    Const1A = 0x1D,
    Const2A = 0x1E,
    Const3A = 0x1F,
}

#[bitos(32)]
#[derive(Debug, Default)]
pub struct StageConstsPair {
    #[bits(4..9)]
    pub color_a: Constant,
    #[bits(9..14)]
    pub alpha_a: Constant,
    #[bits(14..19)]
    pub color_b: Constant,
    #[bits(19..24)]
    pub alpha_b: Constant,
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
#[derive(Clone, PartialEq, Eq, Hash, Default)]
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

impl std::fmt::Debug for StageColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a = self.input_a();
        let b = self.input_b();
        let c = self.input_c();
        let d = self.input_d();
        let sign = if self.negate() { "-" } else { "+" };
        let bias = self.bias().value();
        let scale = self.scale().value();
        let output = self.output();

        write!(
            f,
            "{output:?}.C = [[{sign}({a:?} * (1.0 - {c:?}) + {b:?} * {c:?})] + {d:?} + {bias}] * {scale}"
        )
    }
}

#[bitos(32)]
#[derive(Clone, PartialEq, Eq, Hash, Default)]
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

impl std::fmt::Debug for StageAlpha {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a = self.input_a();
        let b = self.input_b();
        let c = self.input_c();
        let d = self.input_d();
        let sign = if self.negate() { "-" } else { "+" };
        let bias = self.bias().value();
        let scale = self.scale().value();
        let output = self.output();

        write!(
            f,
            "{output:?}.A = [[{sign}({a:?} * (1.0 - {c:?}) + {b:?} * {c:?})] + {d:?} + {bias}] * {scale}"
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct StageOps {
    pub color: StageColor,
    pub alpha: StageAlpha,
}

#[derive(Debug, Default)]
pub struct Interface {
    pub active_stages: u8,
    pub active_channels: u8,
    pub stage_ops: [StageOps; 16],
    pub stage_refs: [StageRefsPair; 8],
    pub stage_consts: [StageConstsPair; 8],
    pub constants: [Rgba; 4],
}
