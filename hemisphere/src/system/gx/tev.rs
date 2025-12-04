//! Texture Environment (TEV).
use bitos::{
    bitos,
    integer::{u2, u3},
};

use crate::system::gx::colors::Rgba;

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
    ChanColor = 0xA,
    ChanAlpha = 0xB,
    One = 0xC,
    Half = 0xD,
    Constant = 0xE,
    Zero = 0xF,
}

impl std::fmt::Display for ColorInputSrc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::R3Color => "R3.C",
            Self::R3Alpha => "R3.A",
            Self::R0Color => "R0.C",
            Self::R0Alpha => "R0.A",
            Self::R1Color => "R1.C",
            Self::R1Alpha => "R1.A",
            Self::R2Color => "R2.C",
            Self::R2Alpha => "R2.A",
            Self::TexColor => "Tex.C",
            Self::TexAlpha => "Tex.A",
            Self::ChanColor => "Channel.C",
            Self::ChanAlpha => "Channel.A",
            Self::One => "1",
            Self::Half => "0.5",
            Self::Constant => "Constant",
            Self::Zero => "0",
        })
    }
}

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaInputSrc {
    R3Alpha = 0x0,
    R0Alpha = 0x1,
    R1Alpha = 0x2,
    R2Alpha = 0x3,
    TexAlpha = 0x4,
    ChanAlpha = 0x5,
    Constant = 0x6,
    Zero = 0x7,
}

impl std::fmt::Display for AlphaInputSrc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::R3Alpha => "R3.A",
            Self::R0Alpha => "R0.A",
            Self::R1Alpha => "R1.A",
            Self::R2Alpha => "R2.A",
            Self::TexAlpha => "Tex.A",
            Self::ChanAlpha => "Channel.A",
            Self::Constant => "Constant",
            Self::Zero => "0",
        })
    }
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bias {
    Zero = 0b00,
    PositiveHalf = 0b01,
    NegativeHalf = 0b10,
    Comparative = 0b11,
}

impl Bias {
    pub fn value(self) -> f32 {
        match self {
            Self::Zero => 0.0,
            Self::PositiveHalf => 0.5,
            Self::NegativeHalf => -0.5,
            _ => panic!("comparative tev stage"),
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

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    GreaterThan,
    Equal,
}

impl std::fmt::Display for CompareOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GreaterThan => f.write_str(">"),
            Self::Equal => f.write_str("=="),
        }
    }
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareTarget {
    R8 = 0b00,
    GR16 = 0b01,
    BGR16 = 0b10,
    Component = 0b11,
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
    #[bits(18)]
    pub compare_op: CompareOp,
    #[bits(19)]
    pub clamp: bool,
    #[bits(20..22)]
    pub scale: Scale,
    #[bits(20..22)]
    pub compare_target: CompareTarget,
    #[bits(22..24)]
    pub output: OutputDst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageColorPattern {
    PassChanColor,
    PassChanAlpha,
    PassTexColor,
    PassTexAlpha,
    Modulate,
    ModulateDouble,
    Add,
    SubTexFromColor,
    SubColorFromTex,
    Mix,
}

impl StageColor {
    pub fn is_comparative(&self) -> bool {
        self.bias() == Bias::Comparative
    }

    pub fn pattern(&self) -> Option<StageColorPattern> {
        use ColorInputSrc as Input;
        use StageColorPattern as Pattern;

        if self.is_comparative() {
            return None;
        }

        let inputs = (
            self.input_a(),
            self.input_b(),
            self.input_c(),
            self.input_d(),
        );
        let positive = !self.negate();
        let scale = self.scale().value();
        let bias = self.bias().value();

        let no_scale_no_bias = scale == 1.0 && bias == 0.0;
        let simple = positive && no_scale_no_bias;

        Some(match inputs {
            (Input::Zero, Input::Zero, Input::Zero, Input::ChanColor) if simple => {
                Pattern::PassChanColor
            }
            (Input::Zero, Input::Zero, Input::Zero, Input::ChanAlpha) if simple => {
                Pattern::PassChanAlpha
            }
            (Input::Zero, Input::Zero, Input::Zero, Input::TexColor) if simple => {
                Pattern::PassTexColor
            }
            (Input::Zero, Input::Zero, Input::Zero, Input::TexAlpha) if simple => {
                Pattern::PassTexAlpha
            }
            (Input::Zero, Input::TexColor, Input::ChanColor, Input::Zero) if simple => {
                Pattern::Modulate
            }
            (Input::Zero, Input::ChanColor, Input::TexColor, Input::Zero) if simple => {
                Pattern::Modulate
            }
            (Input::Zero, Input::TexColor, Input::ChanColor, Input::Zero)
                if positive && scale == 2.0 && bias == 0.0 =>
            {
                Pattern::ModulateDouble
            }
            (Input::Zero, Input::ChanColor, Input::TexColor, Input::Zero)
                if positive && scale == 2.0 && bias == 0.0 =>
            {
                Pattern::ModulateDouble
            }
            (Input::TexColor, Input::Zero, Input::Zero, Input::ChanColor) if simple => Pattern::Add,
            (Input::ChanColor, Input::Zero, Input::Zero, Input::TexColor) if simple => Pattern::Add,
            (Input::TexColor, Input::Zero, Input::Zero, Input::ChanColor)
                if no_scale_no_bias && !positive =>
            {
                Pattern::SubTexFromColor
            }
            (Input::ChanColor, Input::Zero, Input::Zero, Input::TexColor)
                if no_scale_no_bias && !positive =>
            {
                Pattern::SubColorFromTex
            }
            (Input::TexColor, Input::ChanColor, Input::TexAlpha, Input::Zero) if simple => {
                Pattern::Mix
            }
            (Input::ChanColor, Input::TexColor, Input::ChanAlpha, Input::Zero) if simple => {
                Pattern::Mix
            }
            _ => return None,
        })
    }
}

impl std::fmt::Debug for StageColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pattern = self
            .pattern()
            .map(|p| format!("[{p:?}] "))
            .unwrap_or(String::new());

        if self.is_comparative() {
            let a = self.input_a();
            let b = self.input_b();
            let c = self.input_c();
            let d = self.input_d();
            let op = self.compare_op();
            let target = self.compare_target();
            let output = self.output();

            write!(
                f,
                "{output:?}.C = {pattern}({a}.{target:?} {op} {b}.{target:?}) ? {c} : {d}"
            )
        } else {
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
                "{output:?}.C = {pattern}{scale} * ({sign}mix({a}, {b}, {c}) + {d} + {bias})"
            )
        }
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
    #[bits(18)]
    pub compare_op: CompareOp,
    #[bits(19)]
    pub clamp: bool,
    #[bits(20..22)]
    pub scale: Scale,
    #[bits(20..22)]
    pub compare_target: CompareTarget,
    #[bits(22..24)]
    pub output: OutputDst,
}

impl std::fmt::Debug for StageAlpha {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_comparative() {
            let a = self.input_a();
            let b = self.input_b();
            let c = self.input_c();
            let d = self.input_d();
            let op = self.compare_op();
            let target = self.compare_target();
            let output = self.output();

            write!(
                f,
                "{output:?}.A = ({a}.{target:?} {op} {b}.{target:?}) ? {c} : {d}"
            )
        } else {
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
                "{output:?}.A = {scale} * ({sign}mix({a}, {b}, {c}) + {d} + {bias})"
            )
        }
    }
}

impl StageAlpha {
    pub fn is_comparative(&self) -> bool {
        self.bias() == Bias::Comparative
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct StageOps {
    pub color: StageColor,
    pub alpha: StageAlpha,
}

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AlphaCompare {
    #[default]
    Never = 0x0,
    Less = 0x1,
    Equal = 0x2,
    LessOrEqual = 0x3,
    Greater = 0x4,
    NotEqual = 0x5,
    GreaterOrEqual = 0x6,
    Always = 0x7,
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AlphaLogic {
    #[default]
    And = 0b00,
    Or = 0b01,
    Xor = 0b10,
    Xnor = 0b11,
}

#[bitos(32)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct AlphaFunction {
    #[bits(0..16)]
    pub refs: [u8; 2],
    #[bits(16..22)]
    pub comparison: [AlphaCompare; 2],
    #[bits(22..24)]
    pub logic: AlphaLogic,
}

#[derive(Debug, Default)]
pub struct Interface {
    pub active_stages: u8,
    pub active_channels: u8,
    pub stage_ops: [StageOps; 16],
    pub stage_refs: [StageRefsPair; 8],
    pub stage_consts: [StageConstsPair; 8],
    pub constants: [Rgba; 4],
    pub alpha_function: AlphaFunction,
}
