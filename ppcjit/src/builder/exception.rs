use super::BlockBuilder;
use common::arch::{Reg, SPR, disasm::Ins};
use cranelift::prelude::InstBuilder;

impl BlockBuilder<'_> {
    pub fn rfi(&mut self, _: Ins) {
        let msr = self.get(Reg::MSR);
        let srr0 = self.get(SPR::SRR0);
        let srr1 = self.get(SPR::SRR1);
        let mask = self.const_val(0b1000_0111_1100_0000_1111_1111_0111_0011_u32);

        // move only some bits from srr1
        let new_msr = self.bd.ins().bitselect(mask, srr1, msr);

        // clear bit 18
        let new_msr = self.bd.ins().band_imm(new_msr, !(1 << 18));

        // TODO: deal with new_msr exceptions enabled

        let new_pc = self.bd.ins().band_imm(srr0, !0b11);
        let new_pc = self.bd.ins().iadd_imm(new_pc, -4);

        self.set(Reg::PC, new_pc);
        self.set(Reg::MSR, new_msr);
    }
}
