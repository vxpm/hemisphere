use bitos::{
    bitos,
    integer::{u4, u7, u9, u10, u15},
};
use common::Address;

#[bitos(16)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VerticalTiming {
    /// Length of the equalization pulse, in halflines.
    #[bits(0..4)]
    pub equalization_pulse: u4,
    /// Amount of scan lines in the active video of a field.
    #[bits(4..14)]
    pub active_video_lines: u10,
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, Default)]
pub enum DisplayLatchMode {
    #[default]
    Off = 0,
    Once = 1,
    Twice = 2,
    Always = 3,
}

#[bitos(2)]
#[derive(Debug, Clone, Copy, Default)]
pub enum VideoFormat {
    #[default]
    NTSC = 0,
    Pal50 = 1,
    Pal60 = 2,
    Debug = 3,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayConfig {
    /// Enable video timing generation and data request.
    #[bits(0)]
    pub enable: bool,
    /// Clears all data requests and puts the interface into its idle state.
    #[bits(1)]
    pub reset: bool,
    /// Whether progressive video mode is enabled (interlaced otherwise).
    #[bits(2)]
    pub progressive: bool,
    /// Whether the 3D display mode is enabled. This is _not_ 3D rendering - it is a stereoscopic
    /// 3D effect.
    #[bits(3)]
    pub display_mode_3d: bool,
    #[bits(4..6)]
    pub display_latch0_mode: DisplayLatchMode,
    #[bits(6..8)]
    pub display_latch1_mode: DisplayLatchMode,
    /// Current video format.
    #[bits(8..10)]
    pub video_format: VideoFormat,
}

#[bitos(64)]
#[derive(Debug, Clone, Copy, Default)]
pub struct HorizontalTiming {
    // HTR1
    /// Width of the HSync pulse, in samples.
    #[bits(0..7)]
    pub sync_width: u7,
    /// Amount of samples between the start of HSync pulse and HBlank end.
    #[bits(7..17)]
    pub sync_start_to_blank_end: u10,
    /// Amount of samples between the half of the line and HBlank start.
    #[bits(17..27)]
    pub halflline_to_blank_start: u10,

    // HTR0
    /// Width of a halfline, in samples.
    #[bits(32..41)]
    pub halfline_width: u9,
    /// Amount of samples between the start of HSync pulse and color burst end.
    #[bits(48..55)]
    pub sync_start_to_color_burst_end: u7,
    /// Amount of samples between the start of HSync pulse and color burst start.
    #[bits(56..63)]
    pub sync_start_to_color_burst_start: u7,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FieldVerticalTiming {
    /// Length of the pre-blanking interval in half-lines.
    #[bits(0..10)]
    pub pre_blanking: u10,
    /// Length of the post-blanking interval in half-lines.
    #[bits(16..26)]
    pub post_blanking: u10,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FieldBase {
    /// Bits 9..24 of the XFB address for this field.
    #[bits(9..24)]
    pub xfb_address_base: u15,
    #[bits(24..28)]
    pub horizontal_offset: u4,
    /// If set, shifts XFB address right by 5.
    #[bits(28)]
    pub shift_xfb_addr: bool,
}

#[bitos(16)]
#[derive(Debug, Clone, Copy, Default)]
pub struct HorizontalScaling {
    #[bits(0..9)]
    pub step_size: u9,
    #[bits(12)]
    pub enabled: bool,
}

#[bitos(16)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ExternalFramebufferWidth {
    /// Stride of the XFB divided by 16.
    #[bits(0..8)]
    pub stride_by_16: u8,
    /// Width of the XFB divided by 16.
    #[bits(8..15)]
    pub width_by_16: u7,
}

impl ExternalFramebufferWidth {
    /// Stride of the XFB.
    pub fn stride(&self) -> u16 {
        self.stride_by_16() as u16 * 16
    }

    /// Width of the XFB.
    pub fn width(&self) -> u16 {
        self.width_by_16().value() as u16 * 16
    }
}

#[bitos(16)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ClockMode {
    #[bits(0)]
    pub double: bool,
}

impl FieldBase {
    /// Physical address of the XFB for this field.
    pub fn xfb_address(&self) -> Address {
        Address(
            ((self.xfb_address_base().value() as u32) << 9) >> (5 * self.shift_xfb_addr() as usize),
        )
    }
}

#[derive(Debug, Default)]
pub struct Registers {
    pub vertical_timing: VerticalTiming,
    pub display_config: DisplayConfig,
    pub horizontal_timing: HorizontalTiming,
    pub odd_vertical_timing: FieldVerticalTiming,
    pub even_vertical_timing: FieldVerticalTiming,
    pub top_base_left: FieldBase,
    pub top_base_right: u32,
    pub bottom_base_left: FieldBase,
    pub bottom_base_right: u32,
    pub xfb_width: ExternalFramebufferWidth,
    pub horizontal_scaling: HorizontalScaling,
    pub clock: ClockMode,

    pub _2070: u16,
}
