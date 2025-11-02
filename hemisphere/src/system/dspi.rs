use crate::system::System;

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
            self.dsp.reset();

            // DMA from main memory
            if value.reset_high() {
                let data = zerocopy::transmute_ref!(&self.mem.ram[0x0100_0000..][..1024]);
                self.dsp.mem.iram[..512].copy_from_slice(data);
            }
        }
    }
}
