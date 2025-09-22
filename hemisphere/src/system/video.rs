mod regs;

use super::{Event as SystemEvent, processor::Interrupt};
use crate::system::System;
use common::{Address, arch::FREQUENCY};

pub use regs::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    HSync,
    VSync,
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

    /// Address of the XFB for the top field.
    pub fn top_xfb_address(&self) -> Address {
        self.regs.top_base_left.xfb_address()
    }

    /// Address of the XFB for the bottom field.
    pub fn bottom_xfb_address(&self) -> Address {
        self.regs.bottom_base_left.xfb_address()
    }

    /// Resolution of the XFB.
    pub fn xfb_resolution(&self) -> (u16, u16) {
        (
            self.regs.xfb_width.width(),
            self.regs.vertical_timing.active_video_lines().value(),
        )
    }

    pub fn write_interrupt<const N: usize>(&mut self, new: DisplayInterrupt) {
        const { assert!(N < 4) };
        self.regs.interrupts[N] =
            new.with_status(self.regs.interrupts[N].status() && !new.status());
    }
}

impl System {
    pub fn update_video_interface(&mut self) {
        self.scheduler
            .retain(|e| !matches!(e.event, SystemEvent::Video(_)));

        if self.bus.video.regs.display_config.enable() {
            self.process(SystemEvent::Video(Event::HSync));
            self.process(SystemEvent::Video(Event::VSync));
        }
    }

    pub fn check_display_interrupts(&mut self) {
        for (index, interrupt) in self.bus.video.regs.interrupts.clone().iter().enumerate() {
            if interrupt.enable() {
                if interrupt.vertical_count().value() == self.bus.video.regs.vertical_count {
                    self.bus.video.regs.interrupts[index].set_status(true);
                    self.bus.processor.raise_interrupt(Interrupt::Video);
                    self.check_external_interrupts();
                }
            }
        }
    }
}
