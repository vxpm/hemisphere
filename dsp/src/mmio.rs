use bitos::{
    bitos,
    integer::{u15, u31},
};

#[bitos(32)]
#[derive(Debug, Default)]
pub struct Mailbox {
    #[bits(0..16)]
    pub low: u16,
    #[bits(16..31)]
    pub high: u15,
    #[bits(16..32)]
    pub high_and_status: u16,

    #[bits(0..31)]
    pub data: u31,
    #[bits(31)]
    pub status: bool,
}

#[bitos(16)]
#[derive(Debug, Clone, Copy)]
pub struct Control {
    #[bits(0)]
    pub reset: bool,
    #[bits(1)]
    pub interrupt: bool,
    #[bits(2)]
    pub halt: bool,
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
    pub dsp_dma_ongoing: bool,
    #[bits(10)]
    pub unknown: bool,
    #[bits(11)]
    pub reset_high: bool,
}

impl Default for Control {
    fn default() -> Self {
        Self::from_bits(0).with_reset_high(true)
    }
}

impl Control {
    pub fn any_interrupt(&self) -> bool {
        let ai = self.ai_interrupt() && self.ai_interrupt_mask();
        let aram = self.aram_interrupt() && self.aram_interrupt_mask();
        let dsp = self.dsp_interrupt() && self.dsp_interrupt_mask();
        ai || aram || dsp
    }
}

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AramDmaDirection {
    FromRamToAram = 0,
    FromAramToRam = 1,
}

#[bitos(32)]
#[derive(Debug, Clone, Default)]
pub struct AramDmaControl {
    #[bits(0..31)]
    pub length: u31,
    #[bits(31)]
    pub direction: AramDmaDirection,
}

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DspDmaDirection {
    FromRamToDsp = 0,
    FromDspToRam = 1,
}

#[bitos(1)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DspDmaTarget {
    Dmem = 0,
    Imem = 1,
}

#[bitos(16)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DspDmaControl {
    #[bits(0)]
    pub direction: DspDmaDirection,
    #[bits(1)]
    pub dsp_target: DspDmaTarget,
    #[bits(2)]
    pub transfer_ongoing: bool,
}

#[derive(Default)]
pub struct DspDma {
    pub ram_base: u32,
    pub dsp_base: u16,
    pub length: u16,
    pub control: DspDmaControl,
}

#[derive(Default)]
pub struct AramDma {
    pub ram_base: Address,
    pub aram_base: u32,
    pub control: AramDmaControl,
}

#[derive(Default)]
pub struct Mmio {
    pub control: Control,
    /// Data from DSP to CPU
    pub dsp_mailbox: Mailbox,
    /// Data from CPU to DSP
    pub cpu_mailbox: Mailbox,
    pub dsp_dma: DspDma,
    pub aram_dma: AramDma,
}
