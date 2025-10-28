use crate::system::System;
use bitos::bitos;

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleRate {
    KHz48 = 0,
    KHz32 = 1,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Control {
    #[bits(0)]
    pub playing: bool,
    #[bits(1)]
    pub aux_sample_rate: SampleRate,
    #[bits(2)]
    pub interrupt_mask: bool,
    #[bits(3)]
    pub interrupt: bool,
    #[bits(4)]
    pub interrupt_valid: bool,
    #[bits(5)]
    pub sample_counter_reset: bool,
    #[bits(6)]
    pub dsp_sample_rate: SampleRate,
}

#[derive(Default)]
pub struct Interface {
    pub control: Control,
    pub sample_counter: u32,
}

impl Interface {
    pub fn write_control(&mut self, value: Control) {
        self.control.set_playing(value.playing());
        self.control.set_aux_sample_rate(value.aux_sample_rate());
        self.control.set_interrupt_mask(value.interrupt_mask());
        self.control
            .set_interrupt_mask(self.control.interrupt() & !value.interrupt());
        self.control.set_interrupt_valid(value.interrupt_valid());

        if value.sample_counter_reset() {
            self.sample_counter = 0;
        }

        self.control.set_dsp_sample_rate(value.dsp_sample_rate());
    }
}

impl System {}
