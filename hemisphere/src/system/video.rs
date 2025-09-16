mod regs;

use common::arch::FREQUENCY;

pub use regs::*;

#[derive(Debug, Default)]
pub struct VideoInterface {
    pub regs: Registers,
}

impl VideoInterface {
    /// The current video clock.
    pub fn video_clock(&self) -> u32 {
        if self.regs.clock.double() {
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
        self.cycles_per_sample() * self.regs.horizontal_timing.halfline_width().value() as u32
    }

    /// How many halflines long an even field is.
    pub fn halflines_per_even_field(&self) -> u32 {
        3 * self.regs.vertical_timing.eq_pulse().value() as u32
            + self.regs.even_field_vertical_timing.pre_blanking().value() as u32
            + 2 * self.regs.vertical_timing.lines_per_field().value() as u32
            + self.regs.even_field_vertical_timing.post_blanking().value() as u32
    }

    /// How many CPU cycles long an even field is.
    pub fn cycles_per_even_field(&self) -> u32 {
        self.cycles_per_halfline() * self.halflines_per_even_field()
    }

    /// How many halflines long an odd field is.
    pub fn halflines_per_odd_field(&self) -> u32 {
        3 * self.regs.vertical_timing.eq_pulse().value() as u32
            + self.regs.odd_field_vertical_timing.pre_blanking().value() as u32
            + 2 * self.regs.vertical_timing.lines_per_field().value() as u32
            + self.regs.odd_field_vertical_timing.post_blanking().value() as u32
    }

    /// How many CPU cycles long an odd field is.
    pub fn cycles_per_odd_field(&self) -> u32 {
        self.cycles_per_halfline() * self.halflines_per_odd_field()
    }

    /// The refresh rate of the video signal.
    pub fn refresh_rate(&self) -> f64 {
        let cycles_per_field_pair = self.cycles_per_even_field() + self.cycles_per_odd_field();
        2.0 * FREQUENCY as f64 / cycles_per_field_pair as f64
    }
}
