use super::BlockBuilder;
use common::arch::{InsExt, SPR, disasm::Ins};
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, IntCC},
};

impl BlockBuilder<'_> {
    fn compare_signed(&mut self, a: ir::Value, b: ir::Value, index: u8) {
        let xer = self.get(SPR::XER);

        let lt = self.bd.ins().icmp(IntCC::SignedLessThan, a, b);
        let gt = self.bd.ins().icmp(IntCC::SignedGreaterThan, a, b);
        let eq = self.bd.ins().icmp(IntCC::Equal, a, b);
        let ov = self.bd.ins().ishl_imm(xer, 31);

        // reduce OV as update_cr expects a bool
        let ov = self.bd.ins().ireduce(ir::types::I8, ov);

        self.update_cr(index, lt, gt, eq, ov);
    }

    fn compare_unsigned(&mut self, a: ir::Value, b: ir::Value, index: u8) {
        let xer = self.get(SPR::XER);

        let lt = self.bd.ins().icmp(IntCC::UnsignedLessThan, a, b);
        let gt = self.bd.ins().icmp(IntCC::UnsignedGreaterThan, a, b);
        let eq = self.bd.ins().icmp(IntCC::Equal, a, b);
        let ov = self.bd.ins().ishl_imm(xer, 31);

        // reduce OV as update_cr expects a bool
        let ov = self.bd.ins().ireduce(ir::types::I8, ov);

        self.update_cr(index, lt, gt, eq, ov);
    }

    pub fn cmp(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());

        self.compare_signed(ra, rb, ins.field_crfd());
    }

    pub fn cmpi(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let imm = self.const_val(ins.field_simm() as i32);

        self.compare_signed(ra, imm, ins.field_crfd());
    }

    pub fn cmpl(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());

        self.compare_unsigned(ra, rb, ins.field_crfd());
    }

    pub fn cmpli(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let imm = self.const_val(ins.field_uimm() as u32);

        self.compare_unsigned(ra, imm, ins.field_crfd());
    }
}
