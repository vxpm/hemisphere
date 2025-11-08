use crate::system::System;
use bitos::{
    bitos,
    integer::{u15, u31},
};
use gekko::Address;

const fn convert_to_dsp_words<const N: usize>(bytes: &[u8]) -> [u16; N] {
    assert!(bytes.len() / 2 == N);

    let mut result = [0; N];
    let mut i = 0;
    loop {
        if i == N {
            break;
        }

        result[i] = u16::from_be_bytes([bytes[2 * i], bytes[2 * i + 1]]);
        i += 1;
    }

    result
}

pub static DSP_ROM: [u16; 4096] =
    convert_to_dsp_words(include_bytes!("../../../resources/dsp_rom.bin"));

pub static DSP_COEF: [u16; 2048] =
    convert_to_dsp_words(include_bytes!("../../../resources/dsp_coef.bin"));

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
pub struct Dsp {
    pub control: Control,
    /// Data from DSP to CPU
    pub dsp_mailbox: Mailbox,
    /// Data from CPU to DSP
    pub cpu_mailbox: Mailbox,
    pub dsp_dma: DspDma,
    pub aram_dma: AramDma,
}

impl System {
    pub fn dsp_write_control(&mut self, value: Control) {
        self.dsp.control.set_halt(value.halt());

        // DSP external interrupt
        self.dsp.control.set_interrupt(value.interrupt());

        // PI DMA interrupts
        self.dsp
            .control
            .set_ai_interrupt(self.dsp.control.ai_interrupt() & !value.ai_interrupt());
        self.dsp
            .control
            .set_ai_interrupt_mask(value.ai_interrupt_mask());

        self.dsp
            .control
            .set_aram_interrupt(self.dsp.control.aram_interrupt() & !value.aram_interrupt());
        self.dsp
            .control
            .set_aram_interrupt_mask(value.aram_interrupt_mask());

        self.dsp
            .control
            .set_dsp_interrupt(self.dsp.control.dsp_interrupt() & !value.dsp_interrupt());
        self.dsp
            .control
            .set_dsp_interrupt_mask(value.dsp_interrupt_mask());

        self.dsp.control.set_unknown(value.unknown());
        self.dsp.control.set_reset_high(value.reset_high());

        // if value.reset() || (!value.reset_high() && old_reset_high) {
        //     // DMA from main memory if resetting at low
        //     if !value.reset_high() {
        //         tracing::debug!("DSP DMA stub from main memory");
        //         let data = self.mem.ram[0x0100_0000..][..1024]
        //             .chunks_exact(2)
        //             .map(|c| u16::from_be_bytes([c[0], c[1]]));
        //
        //         for (word, data) in self.dsp.mem.iram[..512].iter_mut().zip(data) {
        //             *word = data;
        //         }
        //     }
        //
        //     tracing::debug!("DSP reset");
        //     self.dsp.reset();
        // }
    }

    ///// Performs the ARAM DMA if length is not zero.
    //pub fn dsp_aram_dma(&mut self) {
    //    let length = 4 * self.dsp.aram_dma.control.length().value() as usize;
    //    if length != 0 {
    //        let ram_base = self
    //            .mmu
    //            .translate_data_addr(self.dsp.aram_dma.ram_base.value())
    //            .unwrap_or(self.dsp.aram_dma.ram_base.value());
    //
    //        let aram_base = self.dsp.aram_dma.aram_base & 0x00FF_FFFF;
    //        match self.dsp.aram_dma.control.direction() {
    //            AramDmaDirection::FromRamToAram => {
    //                tracing::debug!(
    //                    "ARAM DMA {length} bytes from RAM {} to ARAM {aram_base:08X}",
    //                    Address(ram_base)
    //                );
    //
    //                self.dsp.mem.aram[aram_base as usize..][..length]
    //                    .copy_from_slice(&self.mem.ram[ram_base as usize..][..length]);
    //            }
    //            AramDmaDirection::FromAramToRam => {
    //                tracing::debug!(
    //                    "ARAM DMA {length} bytes from ARAM {aram_base:08X} to RAM {}",
    //                    Address(ram_base)
    //                );
    //
    //                self.mem.ram[ram_base as usize..][..length]
    //                    .copy_from_slice(&self.dsp.mem.aram[aram_base as usize..][..length]);
    //            }
    //        }
    //
    //        self.dsp.aram_dma.control.set_length(u31::new(0));
    //        self.dsp.control.set_aram_interrupt(true);
    //    }
    //}
    //
    ///// Performs the DSP DMA if the transfer is ongoing.
    //pub fn dsp_dma(&mut self) {
    //    if self.dsp.dsp_dma.control.transfer_ongoing() {
    //        let ram_base = self.dsp.dsp_dma.ram_base & 0xFF_FFFF;
    //        let dsp_base = self.dsp.dsp_dma.dsp_base;
    //        let length = self.dsp.dsp_dma.length;
    //
    //        let (target, direction) = (
    //            self.dsp.dsp_dma.control.dsp_target(),
    //            self.dsp.dsp_dma.control.direction(),
    //        );
    //
    //        match (target, direction) {
    //            (DspDmaTarget::Dmem, DspDmaDirection::FromRamToDsp) => {
    //                tracing::debug!(
    //                    "DSP DMA {length:04X} bytes from RAM {ram_base:08X} to DMEM {dsp_base:04X}"
    //                );
    //
    //                for word in 0..(length / 2) {
    //                    let data = u16::read_be_bytes(
    //                        &self.mem.ram[(ram_base + 2 * word as u32) as usize..],
    //                    );
    //
    //                    self.dsp.write_dmem(dsp_base + word, data);
    //                }
    //            }
    //            (DspDmaTarget::Dmem, DspDmaDirection::FromDspToRam) => {
    //                tracing::debug!(
    //                    "DSP DMA {length:04X} bytes from DMEM {dsp_base:04X} to RAM {ram_base:08X}"
    //                );
    //
    //                for word in 0..(length / 2) {
    //                    let data = self.dsp.read_dmem(dsp_base + word);
    //                    data.write_be_bytes(
    //                        &mut self.mem.ram[(ram_base + 2 * word as u32) as usize..],
    //                    );
    //                }
    //            }
    //            (DspDmaTarget::Imem, DspDmaDirection::FromRamToDsp) => {
    //                tracing::info!(
    //                    "DSP DMA {length:04X} bytes from RAM {ram_base:08X} to IMEM {dsp_base:04X} (ucode)"
    //                );
    //
    //                // let mut file = std::fs::File::create("IMEM.DUMP").unwrap();
    //                for word in 0..(length / 2) {
    //                    let data = u16::read_be_bytes(
    //                        &self.mem.ram[(ram_base + 2 * word as u32) as usize..],
    //                    );
    //
    //                    // file.write_all(data.to_be().as_bytes()).unwrap();
    //                    self.dsp.write_imem(dsp_base + word, data);
    //                }
    //
    //                // panic!()
    //            }
    //            (DspDmaTarget::Imem, DspDmaDirection::FromDspToRam) => {
    //                todo!()
    //            }
    //        };
    //
    //        self.dsp.dsp_dma.length = 0;
    //        self.dsp.dsp_dma.control.set_transfer_ongoing(false);
    //        self.dsp.control.set_dsp_dma_ongoing(false);
    //    }
    //}
}
