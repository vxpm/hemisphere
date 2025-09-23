mod regs;

use super::{Event as SystemEvent, processor::Interrupt};
use crate::system::System;
use common::{Address, arch::FREQUENCY};

pub use regs::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    HSync,
}

#[derive(Debug, Default)]
pub struct Interface {
    pub regs: Registers,
}

impl Interface {
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
        3 * self.regs.vertical_timing.equalization_pulse().value() as u32
            + self.regs.even_vertical_timing.pre_blanking().value() as u32
            + 2 * self.regs.vertical_timing.active_video_lines().value() as u32
            + self.regs.even_vertical_timing.post_blanking().value() as u32
    }

    /// How many CPU cycles long an even field is.
    pub fn cycles_per_even_field(&self) -> u32 {
        self.cycles_per_halfline() * self.halflines_per_even_field()
    }

    /// How many halflines long an odd field is.
    pub fn halflines_per_odd_field(&self) -> u32 {
        3 * self.regs.vertical_timing.equalization_pulse().value() as u32
            + self.regs.odd_vertical_timing.pre_blanking().value() as u32
            + 2 * self.regs.vertical_timing.active_video_lines().value() as u32
            + self.regs.odd_vertical_timing.post_blanking().value() as u32
    }

    /// How many CPU cycles long an odd field is.
    pub fn cycles_per_odd_field(&self) -> u32 {
        self.cycles_per_halfline() * self.halflines_per_odd_field()
    }

    /// The refresh rate of the video output.
    pub fn refresh_rate(&self) -> f64 {
        let cycles_per_field_pair = self.cycles_per_even_field() + self.cycles_per_odd_field();
        2.0 * FREQUENCY as f64 / cycles_per_field_pair as f64
    }

    pub fn cycles_until_hsync(&self) -> u32 {
        let halfline_to_blank_start = (self
            .regs
            .horizontal_timing
            .halfline_to_blank_start()
            .value() as u32
            + self.regs.horizontal_timing.sync_width().value() as u32)
            * self.cycles_per_sample();

        self.cycles_per_halfline() + halfline_to_blank_start
    }

    /// Address of the XFB for the top field.
    pub fn top_xfb_address(&self) -> Address {
        self.regs.top_base_left.xfb_address()
    }

    /// Address of the XFB for the bottom field.
    pub fn bottom_xfb_address(&self) -> Address {
        self.regs.bottom_base_left.xfb_address()
    }

    /// Height of the XFB.
    pub fn xfb_height(&self) -> u16 {
        let acv = self.regs.vertical_timing.active_video_lines().value();
        let height_multiplier = if self.regs.display_config.progressive() {
            1
        } else {
            2
        };

        height_multiplier * acv
    }

    /// Width of the XFB.
    pub fn xfb_width(&self) -> u16 {
        let width = self.regs.xfb_width.width();
        if width != 0 {
            width
        } else {
            self.regs.horizontal_timing.halfline_width().value()
                + self
                    .regs
                    .horizontal_timing
                    .halfline_to_blank_start()
                    .value()
                - self
                    .regs
                    .horizontal_timing
                    .sync_start_to_blank_end()
                    .value()
        }
    }

    /// Resolution of the XFB.
    pub fn xfb_resolution(&self) -> (u16, u16) {
        (self.xfb_width(), self.xfb_height())
    }

    pub fn write_interrupt<const N: usize>(&mut self, new: DisplayInterrupt) {
        const { assert!(N < 4) };
        self.regs.interrupts[N] = new.with_status(self.regs.interrupts[N].status() && new.status());
    }
}

impl System {
    pub fn update_video_interface(&mut self) {
        self.bus.video.regs.horizontal_count = 1;
        self.bus.video.regs.vertical_count = 1;

        self.scheduler
            .retain(|e| !matches!(e.event, SystemEvent::Video(_)));

        if self.bus.video.regs.display_config.enable() {
            self.process(SystemEvent::Video(Event::HSync));
        }
    }

    pub fn check_display_interrupts(&mut self) {
        let mut raised = false;
        for (index, interrupt) in self.bus.video.regs.interrupts.iter_mut().enumerate() {
            if interrupt.enable()
                && interrupt.vertical_count().value() == self.bus.video.regs.vertical_count
            {
                raised = true;
                interrupt.set_status(true);
                self.bus.processor.raise_interrupt(Interrupt::Video);
                tracing::debug!("raised display interrupt {index} ({interrupt:?})");
            }
        }

        if raised {
            self.check_external_interrupts();
        }
    }
}
