use crate::system::gpu::colors::Abgr8;
use bitos::{
    bitos,
    integer::{u2, u10},
};

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    RGB8Z24 = 0x0,
    RGBA6Z24 = 0x1,
    RGB565Z16 = 0x2,
    Z24 = 0x3,
    Y8 = 0x4,
    U8 = 0x5,
    V8 = 0x6,
    YUV420 = 0x7,
}

impl Format {
    pub fn has_alpha(self) -> bool {
        self == Self::RGBA6Z24
    }
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DepthCompression {
    #[default]
    Linear = 0b00,
    Near = 0b01,
    Mid = 0b10,
    Far = 0b11,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Control {
    #[bits(0..3)]
    pub format: Format,
    #[bits(3..5)]
    pub depth_compression: DepthCompression,
    #[bits(6)]
    pub depth_compress_before_tex: bool,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConstantAlpha {
    #[bits(0..8)]
    pub value: u8,
    #[bits(8)]
    pub enabled: bool,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CopySrc {
    #[bits(0..10)]
    pub x: u10,
    #[bits(10..20)]
    pub y: u10,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CopyDimensions {
    #[bits(0..10)]
    pub width_minus_one: u10,
    #[bits(10..20)]
    pub height_minus_one: u10,
}

#[bitos(32)]
#[derive(Debug, Default)]
pub struct CopyCmd {
    #[bits(0..2)]
    pub clamp: u2,
    #[bits(4..7)]
    pub format: Format,
    #[bits(7..9)]
    pub gamma: u2,
    #[bits(11)]
    pub clear: bool,
    #[bits(14)]
    pub to_xfb: bool,
}

#[bitos(16)]
#[derive(Debug, Default)]
pub struct InterruptStatus {
    #[bits(2)]
    pub token: bool,
    #[bits(3)]
    pub finish: bool,
}

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompareMode {
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

#[bitos(32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DepthMode {
    #[bits(0)]
    pub enable: bool,
    #[bits(1..4)]
    pub compare: CompareMode,
    #[bits(4)]
    pub update: bool,
}

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SrcBlendFactor {
    #[default]
    Zero = 0x0,
    One = 0x1,
    DstColor = 0x2,
    InverseDstColor = 0x3,
    SrcAlpha = 0x4,
    InverseSrcAlpha = 0x5,
    DstAlpha = 0x6,
    InverseDstAlpha = 0x7,
}

#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DstBlendFactor {
    #[default]
    Zero = 0x0,
    One = 0x1,
    SrcColor = 0x2,
    InverseSrcColor = 0x3,
    SrcAlpha = 0x4,
    InverseSrcAlpha = 0x5,
    DstAlpha = 0x6,
    InverseDstAlpha = 0x7,
}

#[bitos(4)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendLogicOp {
    #[default]
    Clear = 0x0,
    And = 0x1,
    ReverseAnd = 0x2,
    Copy = 0x3,
    InverseAnd = 0x4,
    Noop = 0x5,
    Xor = 0x6,
    Or = 0x7,
    Nor = 0x8,
    Equiv = 0x9,
    Inverse = 0xA,
    ReverseOr = 0xB,
    InverseCopy = 0xC,
    InverseOr = 0xD,
    Nand = 0xE,
    Set = 0xF,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlendMode {
    #[bits(0)]
    pub enable: bool,
    #[bits(1)]
    pub logic_op_enable: bool,
    #[bits(2)]
    pub dither_enable: bool,
    #[bits(3)]
    pub color_mask: bool,
    #[bits(4)]
    pub alpha_mask: bool,
    #[bits(5..8)]
    pub dst_factor: DstBlendFactor,
    #[bits(8..11)]
    pub src_factor: SrcBlendFactor,
    #[bits(11)]
    pub blend_subtract: bool,
    #[bits(12..16)]
    pub logic_op: BlendLogicOp,
}

#[derive(Debug, Default)]
pub struct Interface {
    pub control: Control,
    pub interrupt: InterruptStatus,
    pub constant_alpha: ConstantAlpha,
    pub copy_src: CopySrc,
    pub copy_dimensions: CopyDimensions,
    pub clear_color: Abgr8,
    pub clear_depth: u32,
    pub depth_mode: DepthMode,
    pub blend_mode: BlendMode,
    pub token: u32,
}

impl Interface {
    pub fn write_interrupt(&mut self, status: u16) {
        self.interrupt = InterruptStatus::from_bits(self.interrupt.to_bits() & !status)
    }
}
