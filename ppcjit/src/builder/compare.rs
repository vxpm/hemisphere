use super::BlockBuilder;
use crate::builder::{Reg, registers::Spr};
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, IntCC},
};
use powerpc::Ins;

impl BlockBuilder<'_> {
    pub fn cmpi(&mut self, ins: Ins) {
        let ra = self.get(Reg::Gpr(ins.field_ra()));
        let imm = ins.field_simm() as i64;
        let shift_factor = (4 * (7 - ins.field_crfd())) as i64;

        let xer = self.get(Reg::Spr(Spr::XER));
        let ov = self.bd.ins().ushr_imm(xer, 31);

        let lt = self.bd.ins().icmp_imm(IntCC::SignedLessThan, ra, imm);
        let gt = self.bd.ins().icmp_imm(IntCC::SignedGreaterThan, ra, imm);
        let eq = self.bd.ins().icmp_imm(IntCC::Equal, ra, imm);

        let lt = self.bd.ins().uextend(ir::types::I32, lt);
        let gt = self.bd.ins().uextend(ir::types::I32, gt);
        let eq = self.bd.ins().uextend(ir::types::I32, eq);

        let lt = self.bd.ins().ishl_imm(lt, 3);
        let gt = self.bd.ins().ishl_imm(gt, 2);
        let eq = self.bd.ins().ishl_imm(eq, 1);
        let ov = self.bd.ins().ishl_imm(ov, 0);

        let value = self.bd.ins().bor(lt, gt);
        let value = self.bd.ins().bor(value, eq);
        let value = self.bd.ins().bor(value, ov);
        let value = self.bd.ins().ishl_imm(value, shift_factor);

        let mask = self.bd.ins().iconst(ir::types::I32, 0b1111);
        let mask = self.bd.ins().ishl_imm(mask, shift_factor);

        let cr = self.get(Reg::Cr);
        let updated = self.bd.ins().bitselect(mask, value, cr);
        self.set(Reg::Cr, updated);
    }
}
