use super::{Event as SystemEvent, processor::Interrupt};
use crate::system::System;
use bitos::{
    bitos,
    integer::{u4, u7, u9, u10, u24},
};
use common::{Address, arch::FREQUENCY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    VerticalCount,
}

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

#[bitos(16)]
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
    pub stereoscopic_effect: bool,
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
    pub halfline_to_blank_start: u10,

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
    /// Bits 0..24 of the XFB address for this field.
    #[bits(0..24)]
    pub xfb_address_base: u24,
    #[bits(24..28)]
    pub horizontal_offset: u4,
    /// If set, shifts XFB address right by 5.
    #[bits(28)]
    pub shift_xfb_addr: bool,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayInterrupt {
    /// Sample count for the interrupt.
    #[bits(0..9)]
    pub horizontal_count: u9,
    /// Line count for the interrupt.
    #[bits(16..26)]
    pub vertical_count: u10,
    /// Whether this interrupt is enabled.
    #[bits(28)]
    pub enable: bool,
    /// Whether this interrupt is asserted. Clear on write.
    #[bits(31)]
    pub status: bool,
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
        Address((self.xfb_address_base().value()) >> (5 * self.shift_xfb_addr() as usize))
    }
}

#[derive(Debug, Default)]
pub struct Interface {
    pub vertical_timing: VerticalTiming,
    pub display_config: DisplayConfig,
    pub horizontal_timing: HorizontalTiming,
    pub odd_vertical_timing: FieldVerticalTiming,
    pub even_vertical_timing: FieldVerticalTiming,
    pub top_base_left: FieldBase,
    pub top_base_right: u32,
    pub bottom_base_left: FieldBase,
    pub bottom_base_right: u32,
    pub vertical_count: u16,
    pub horizontal_count: u16,
    pub interrupts: [DisplayInterrupt; 4],
    pub xfb_width: ExternalFramebufferWidth,
    pub horizontal_scaling: HorizontalScaling,
    pub clock: ClockMode,
}

impl Interface {
    /// The current video clock.
    pub fn video_clock(&self) -> u32 {
        if self.clock.double() {
            54_000_000
        } else {
            27_000_000
        }
    }

    /// How many CPU cycles long a sample (~ pixel) is.
    pub fn cycles_per_sample(&self) -> u32 {
        2 * FREQUENCY / self.video_clock()
    }

    /// How many CPU cycles long a halfline is.
    pub fn cycles_per_halfline(&self) -> u32 {
        self.cycles_per_sample() * self.horizontal_timing.halfline_width().value() as u32
    }

    /// How many halflines long an even field is.
    pub fn halflines_per_even_field(&self) -> u32 {
        3 * self.vertical_timing.equalization_pulse().value() as u32
            + self.even_vertical_timing.pre_blanking().value() as u32
            + 2 * self.vertical_timing.active_video_lines().value() as u32
            + self.even_vertical_timing.post_blanking().value() as u32
    }

    /// How many CPU cycles long an even field is.
    pub fn cycles_per_even_field(&self) -> u32 {
        self.cycles_per_halfline() * self.halflines_per_even_field()
    }

    /// How many halflines long an odd field is.
    pub fn halflines_per_odd_field(&self) -> u32 {
        3 * self.vertical_timing.equalization_pulse().value() as u32
            + self.odd_vertical_timing.pre_blanking().value() as u32
            + 2 * self.vertical_timing.active_video_lines().value() as u32
            + self.odd_vertical_timing.post_blanking().value() as u32
    }

    /// How many halflines long a frame is.
    pub fn halflines_per_frame(&self) -> u32 {
        self.halflines_per_even_field()
            + self
                .display_config
                .progressive()
                .then_some(0)
                .unwrap_or(self.halflines_per_odd_field())
    }

    /// How many lines long an even field is.
    pub fn lines_per_even_field(&self) -> u32 {
        self.halflines_per_even_field() / 2
    }

    /// How many lines long an even field is.
    pub fn lines_per_odd_field(&self) -> u32 {
        self.halflines_per_odd_field() / 2
    }

    /// How many halflines long a frame is.
    pub fn lines_per_frame(&self) -> u32 {
        self.halflines_per_frame() / 2
    }

    /// How many CPU cycles long an odd field is.
    pub fn cycles_per_odd_field(&self) -> u32 {
        self.cycles_per_halfline() * self.halflines_per_odd_field()
    }

    /// The refresh rate of the video output.
    pub fn refresh_rate(&self) -> f64 {
        let cycles_per_frame = self.cycles_per_even_field() + self.cycles_per_odd_field();
        2.0 * FREQUENCY as f64 / cycles_per_frame as f64
    }

    /// Address of the XFB for the top field.
    pub fn top_xfb_address(&self) -> Address {
        self.top_base_left.xfb_address()
    }

    /// Address of the XFB for the bottom field.
    pub fn bottom_xfb_address(&self) -> Address {
        self.bottom_base_left.xfb_address()
    }

    /// Height of the XFB.
    pub fn xfb_height(&self) -> u16 {
        let acv = self.vertical_timing.active_video_lines().value();
        let height_multiplier = if self.display_config.progressive() {
            1
        } else {
            2
        };

        height_multiplier * acv
    }

    /// Width of the XFB.
    pub fn xfb_width(&self) -> u16 {
        let width = self.xfb_width.width();
        if width != 0 {
            width
        } else {
            self.horizontal_timing.halfline_width().value()
                + self.horizontal_timing.halfline_to_blank_start().value()
                - self.horizontal_timing.sync_start_to_blank_end().value()
        }
    }

    /// Resolution of the XFB.
    pub fn xfb_resolution(&self) -> (u16, u16) {
        (self.xfb_width(), self.xfb_height())
    }

    pub fn write_interrupt<const N: usize>(&mut self, new: DisplayInterrupt) {
        const { assert!(N < 4) };
        self.interrupts[N] = new.with_status(self.interrupts[N].status() && new.status());
    }
}

/// Video Interface
impl System {
    pub fn update_video_interface(&mut self) {
        self.video.horizontal_count = 1;
        self.video.vertical_count = 1;

        self.scheduler
            .retain(|e| !matches!(e.event, SystemEvent::Video(_)));

        if self.video.display_config.enable() {
            self.process(SystemEvent::Video(Event::VerticalCount));
        }
    }

    pub fn check_display_interrupts(&mut self) {
        let mut raised = false;
        for (index, interrupt) in self.video.interrupts.iter_mut().enumerate() {
            if interrupt.enable()
                && interrupt.vertical_count().value() == self.video.vertical_count
            {
                raised = true;
                interrupt.set_status(true);
                self.processor.raise_interrupt(Interrupt::Video);
                tracing::debug!("raised display interrupt {index} ({interrupt:?})");
            }
        }

        if raised {
            self.check_external_interrupts();
        }
    }

    fn xfb_inner(&self, base: Address) -> Option<&[u8]> {
        let xfb = base.value();
        let (width, height) = self.video.xfb_resolution();

        let pixels = width as u32 * height as u32;
        if pixels == 0 {
            return None;
        }

        let length = 2 * pixels;
        Some(&self.mem.ram[xfb as usize..xfb as usize + length as usize])
    }

    /// Returns the data of the top XFB in YCbCr format (y0, cb, y1, cr).
    pub fn top_xfb(&self) -> Option<&[u8]> {
        let base = self.video.top_xfb_address();
        self.xfb_inner(base)
    }

    /// Returns the data of the bottom XFB in YCbCr format (y0, cb, y1, cr).
    pub fn bottom_xfb(&self) -> Option<&[u8]> {
        let base = self.video.bottom_xfb_address();
        self.xfb_inner(base)
    }
}
