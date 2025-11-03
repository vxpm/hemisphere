use crate::system::System;

pub static DSP_ROM: [u16; 4096] = {
    let bytes = include_bytes!("../../../resources/dsp_rom.bin");

    let mut result = [0; 4096];
    let mut i = 0;
    loop {
        if i == 4096 {
            break;
        }

        result[i] = u16::from_be_bytes([bytes[2 * i], bytes[2 * i + 1]]);
        i += 1;
    }

    result
};

pub static DSP_COEF: [u16; 2048] = {
    let bytes = include_bytes!("../../../resources/dsp_coef.bin");

    let mut result = [0; 2048];
    let mut i = 0;
    loop {
        if i == 2048 {
            break;
        }

        result[i] = u16::from_be_bytes([bytes[2 * i], bytes[2 * i + 1]]);
        i += 1;
    }

    result
};

impl System {
    pub fn dsp_write_control(&mut self, value: dsp::mmio::Control) {
        self.dsp.mmio.control.set_halt(value.halt());

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

        self.dsp.mmio.control.set_reset_high(value.reset_high());

        if value.reset() {
            tracing::debug!("DSP reset");
            self.dsp.reset();

            // DMA from main memory
            if !value.reset_high() {
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

    pub fn dsp_aram_dma(&mut self) {
        println!("{:#?} at {}", self.dsp.mmio.aram_dma_control, self.cpu.pc);
    }
}
