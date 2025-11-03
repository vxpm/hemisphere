use crate::system::Event;
use crate::system::System;
use bitos::integer::u31;
use common::Primitive;
use dsp::mmio::{AramDmaDirection, DspDmaDirection, DspDmaTarget};

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

pub static DSP_ROM: [u16; 4096] = {
    let bytes = include_bytes!("../../../resources/dsp_rom.bin");
    convert_to_dsp_words(bytes)
};

pub static DSP_COEF: [u16; 2048] = {
    let bytes = include_bytes!("../../../resources/dsp_coef.bin");
    convert_to_dsp_words(bytes)
};

impl System {
    pub fn dsp_write_control(&mut self, value: dsp::mmio::Control) {
        self.dsp.mmio.control.set_halt(value.halt());

        self.dsp.mmio.control.set_interrupt(value.interrupt());
        self.dsp
            .mmio
            .control
            .set_ai_interrupt(self.dsp.mmio.control.ai_interrupt() & !value.ai_interrupt());
        self.dsp
            .mmio
            .control
            .set_ai_interrupt_mask(value.ai_interrupt_mask());

        self.dsp
            .mmio
            .control
            .set_aram_interrupt(self.dsp.mmio.control.aram_interrupt() & !value.aram_interrupt());
        self.dsp
            .mmio
            .control
            .set_aram_interrupt_mask(value.aram_interrupt_mask());

        self.dsp
            .mmio
            .control
            .set_dsp_interrupt(self.dsp.mmio.control.dsp_interrupt() & !value.dsp_interrupt());
        self.dsp
            .mmio
            .control
            .set_dsp_interrupt_mask(value.dsp_interrupt_mask());

        self.dsp.mmio.control.set_unknown(value.unknown());
        self.dsp.mmio.control.set_reset_high(value.reset_high());

        if value.reset() {
            tracing::debug!("DSP reset");
            self.dsp.reset();

            // DMA from main memory
            if value.reset_high() {
                tracing::debug!("DSP DMA stub from main memory");
                let data = self.mem.ram[0x0100_0000..][..1024]
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]));

                for (word, data) in self.dsp.mem.iram[..512].iter_mut().zip(data) {
                    *word = data;
                }
            }
        }
    }

    /// Performs the ARAM DMA if length is not zero.
    pub fn dsp_aram_dma(&mut self) {
        let length = 4 * self.dsp.mmio.aram_dma.control.length().value() as usize;
        if length != 0 {
            let ram_base = self
                .mmu
                .translate_data_addr(self.dsp.mmio.aram_dma.ram_base.value())
                .unwrap_or(self.dsp.mmio.aram_dma.ram_base.value());

            let aram_base = self.dsp.mmio.aram_dma.aram_base & 0x00FF_FFFF;
            match self.dsp.mmio.aram_dma.control.direction() {
                AramDmaDirection::FromRamToAram => {
                    tracing::debug!(
                        "ARAM DMA {length} bytes from RAM {ram_base} to ARAM {aram_base:08X}"
                    );

                    self.dsp.mem.aram[aram_base as usize..][..length]
                        .copy_from_slice(&self.mem.ram[ram_base as usize..][..length]);
                }
                AramDmaDirection::FromAramToRam => {
                    tracing::debug!(
                        "ARAM DMA {length} bytes from ARAM {aram_base:08X} to RAM {ram_base}"
                    );

                    self.mem.ram[ram_base as usize..][..length]
                        .copy_from_slice(&self.dsp.mem.aram[aram_base as usize..][..length]);
                }
            }

            self.dsp.mmio.aram_dma.control.set_length(u31::new(0));
            self.dsp.mmio.control.set_aram_interrupt(true);
            self.scheduler.schedule_now(Event::CheckInterrupts);
        }
    }

    /// Performs the DSP DMA if length is not zero.
    pub fn dsp_dma(&mut self) {
        if self.dsp.mmio.dsp_dma.length != 0 {
            let ram_base = self
                .mmu
                .translate_data_addr(self.dsp.mmio.dsp_dma.ram_base)
                .unwrap_or(self.dsp.mmio.dsp_dma.ram_base);

            let dsp_base = self.dsp.mmio.dsp_dma.dsp_base;
            let length = self.dsp.mmio.dsp_dma.length / 2;

            let (target, direction) = (
                self.dsp.mmio.dsp_dma.control.dsp_target(),
                self.dsp.mmio.dsp_dma.control.direction(),
            );

            match (target, direction) {
                (DspDmaTarget::Dmem, DspDmaDirection::FromRamToDsp) => {
                    tracing::debug!(
                        "DSP DMA {length:04X} bytes from RAM {ram_base:08X} to DMEM {dsp_base:04X}"
                    );

                    for word in 0..length {
                        let data = u16::read_be_bytes(
                            &self.mem.ram[(ram_base + 2 * word as u32) as usize..],
                        );

                        self.dsp.write_dmem(dsp_base + word, data);
                    }
                }
                (DspDmaTarget::Dmem, DspDmaDirection::FromDspToRam) => {
                    tracing::debug!(
                        "DSP DMA {length:04X} bytes from DMEM {dsp_base:04X} to RAM {ram_base:08X}"
                    );

                    for word in 0..length {
                        let data = self.dsp.read_dmem(dsp_base + word);
                        data.write_be_bytes(
                            &mut self.mem.ram[(ram_base + 2 * word as u32) as usize..],
                        );
                    }
                }
                (DspDmaTarget::Imem, DspDmaDirection::FromRamToDsp) => {
                    tracing::debug!(
                        "DSP DMA {length:04X} bytes from RAM {ram_base:08X} to IMEM {dsp_base:04X}"
                    );

                    for word in 0..length {
                        let data = u16::read_be_bytes(
                            &self.mem.ram[(ram_base + 2 * word as u32) as usize..],
                        );

                        self.dsp.write_imem(dsp_base + word, data);
                    }
                }
                (DspDmaTarget::Imem, DspDmaDirection::FromDspToRam) => {
                    todo!()
                }
            };

            self.dsp.mmio.dsp_dma.length = 0;
        }
    }
}
