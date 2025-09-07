use super::BlockBuilder;
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, IntCC},
};
use hemicore::arch::{Reg, SPR, powerpc::Ins};

impl BlockBuilder<'_> {
    pub fn stub(&mut self, _: Ins) {
        // stub
    }

    pub fn update_xer_ov(&mut self, overflowed: ir::Value) {
        let xer = self.get(SPR::XER);
        let overflowed = self.bd.ins().uextend(ir::types::I32, overflowed);

        let ov = self.bd.ins().ishl_imm(overflowed, 30);
        let so = self.bd.ins().ishl_imm(overflowed, 31);
        let value = self.bd.ins().bor(ov, so);

        let mask = self.bd.ins().iconst(ir::types::I32, !(0b1 << 30));
        let masked = self.bd.ins().band(xer, mask);
        let updated = self.bd.ins().bor(masked, value);

        self.set(SPR::XER, updated);
    }

    pub fn update_xer_ca(&mut self, carry: ir::Value) {
        let xer = self.get(SPR::XER);
        let carry = self.bd.ins().uextend(ir::types::I32, carry);
        let value = self.bd.ins().ishl_imm(carry, 29);

        let mask = self.bd.ins().iconst(ir::types::I32, !(0b1 << 29));
        let masked = self.bd.ins().band(xer, mask);
        let updated = self.bd.ins().bor(masked, value);

        self.set(SPR::XER, updated);
    }

    /// All ir values must be booleans (i.e. I8).
    pub fn update_cr(
        &mut self,
        index: u8,
        lt: ir::Value,
        gt: ir::Value,
        eq: ir::Value,
        ov: ir::Value,
    ) {
        let cr = self.get(Reg::CR);

        let lt = self.bd.ins().uextend(ir::types::I32, lt);
        let gt = self.bd.ins().uextend(ir::types::I32, gt);
        let eq = self.bd.ins().uextend(ir::types::I32, eq);
        let ov = self.bd.ins().uextend(ir::types::I32, ov);

        let base = (4 * (7 - index)) as u64 as i64;
        let lt = self.bd.ins().ishl_imm(lt, base + 3);
        let gt = self.bd.ins().ishl_imm(gt, base + 2);
        let eq = self.bd.ins().ishl_imm(eq, base + 1);
        let ov = self.bd.ins().ishl_imm(ov, base + 0);

        let value = self.bd.ins().bor(lt, gt);
        let value = self.bd.ins().bor(value, eq);
        let value = self.bd.ins().bor(value, ov);

        let mask = self.bd.ins().iconst(ir::types::I32, 0b1111 << base);
        let updated = self.bd.ins().bitselect(mask, value, cr);

        self.set(Reg::CR, updated);
    }

    pub fn update_cr0_implicit(&mut self, value: ir::Value, overflowed: ir::Value) {
        let lt = self.bd.ins().icmp_imm(IntCC::SignedLessThan, value, 0);
        let gt = self.bd.ins().icmp_imm(IntCC::SignedGreaterThan, value, 0);
        let eq = self.bd.ins().icmp_imm(IntCC::Equal, value, 0);

        self.update_cr(0, lt, gt, eq, overflowed);
    }
}
