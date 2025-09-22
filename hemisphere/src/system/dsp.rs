use bitos::{
    bitos,
    integer::{u15, u31},
};
use common::Address;

#[bitos(32)]
#[derive(Debug, Default)]
pub struct Mailbox {
    #[bits(0..31)]
    pub data: u31,
    #[bits(31)]
    pub status: bool,
}

#[bitos(16)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DspControl {
    #[bits(0)]
    pub reset: bool,
    #[bits(1)]
    pub interrupt: bool,
    #[bits(2)]
    pub halted: bool,
    #[bits(3)]
    pub ai_interrupt: bool,
    #[bits(4)]
    pub ai_interrupt_mask: bool,
    #[bits(5)]
    pub aram_interrupt: bool,
    #[bits(6)]
    pub aram_interrupt_mask: bool,
    #[bits(7)]
    pub dsp_interrupt: bool,
    #[bits(8)]
    pub dsp_interrupt_mask: bool,
    #[bits(9)]
    pub aram_dma_ongoing: bool,
    #[bits(11)]
    pub secondary_reset: bool,
}

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AramDmaDirection {
    FromRamToAram = 0,
    FromAramToRam = 1,
}

#[bitos(16)]
#[derive(Debug, Clone, Default)]
pub struct AramDmaControl {
    #[bits(0..15)]
    pub length: u15,
    #[bits(15)]
    pub direction: AramDmaDirection,
}

#[derive(Default)]
pub struct DspInterface {
    pub dsp_mailbox: Mailbox,
    pub cpu_mailbox: Mailbox,
    pub control: DspControl,
    pub aram_dma_ram: Address,
    pub aram_dma_aram: Address,
    pub aram_dma_control: AramDmaControl,
}

impl DspInterface {
    pub fn write_control(&mut self, new: DspControl) {
        if new.reset() {
            self.control = DspControl::default();
        }

        self.control.set_halted(new.halted());

        if new.ai_interrupt() {
            self.control.set_ai_interrupt(false);
        }
        self.control.set_ai_interrupt_mask(new.ai_interrupt_mask());

        if new.aram_interrupt() {
            self.control.set_aram_interrupt(false);
        }
        self.control
            .set_aram_interrupt_mask(new.aram_interrupt_mask());

        if new.dsp_interrupt() {
            self.control.set_dsp_interrupt(false);
        }
        self.control
            .set_dsp_interrupt_mask(new.dsp_interrupt_mask());
    }
}
