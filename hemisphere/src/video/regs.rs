use bitos::{
    bitos,
    integer::{u4, u5, u7, u9, u10, u11, u15},
};
use common::Address;

// LINE = one horizontal line
// FIELD = number of lines in a scan

#[bitos(16)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VerticalTiming {
    /// Length of the equalization pulse in halflines.
    #[bits(0..4)]
    pub eq_pulse: u4,
    /// Active video in lines per field (?)
    #[bits(4..14)]
    pub lines_per_field: u10,
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
    #[bits(8..10)]
    pub video_format: VideoFormat,
}

#[bitos(64)]
#[derive(Debug, Clone, Copy, Default)]
pub struct HorizontalTiming {
    /// The width of a halfline, in samples
    #[bits(0..9)]
    pub halfline_width: u9,
    /// (?) between the start of a HSync and the end of the color burst.
    #[bits(16..23)]
    pub hsync_start_to_color_burst_end: u7,
    /// (?) between the start of a HSync and the start of the color burst.
    #[bits(24..31)]
    pub hsync_start_to_color_burst_start: u7,
    #[bits(32..39)]
    pub hsync_width: u7,
    #[bits(39..48)]
    pub hsync_start_to_hblank_end: u9,
    #[bits(48..58)]
    pub halfline_to_hblank_start: u10,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FieldVerticalTiming {
    /// In half lines
    #[bits(0..10)]
    pub pre_blanking: u10,
    /// In half lines
    #[bits(16..26)]
    pub post_blanking: u10,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FieldBurstBlankingInterval {
    /// In half lines
    #[bits(0..5)]
    pub field_start_to_burst_blanking_start: u5,
    /// In half lines
    #[bits(5..16)]
    pub field_start_to_burst_blanking_end: u11,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FieldBase {
    #[bits(9..24)]
    pub xfb_addr_offset: u15,
    #[bits(24..28)]
    pub horizontal_offset: u4,
    #[bits(28)]
    pub shift_addr: bool,
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
pub struct ClockMode {
    #[bits(0)]
    pub double: bool,
}

impl FieldBase {
    pub fn xfb_address(&self) -> Address {
        Address((self.xfb_addr_offset().value() as u32) << 9)
    }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Registers {
    pub vertical_timing: VerticalTiming,
    pub display_config: DisplayConfig,
    pub horizontal_timing: HorizontalTiming,
    pub odd_field_vertical_timing: FieldVerticalTiming,
    pub even_field_vertical_timing: FieldVerticalTiming,
    pub odd_field_bb_interval: FieldBurstBlankingInterval,
    pub even_field_bb_interval: FieldBurstBlankingInterval,
    pub top_field_base: FieldBase,
    pub tfbr: u32,
    pub bottom_field_base: FieldBase,
    pub bfbr: u32,
    pub horizontal_scaling: HorizontalScaling,
    pub clock: ClockMode,

    pub _2070: u16,
}
