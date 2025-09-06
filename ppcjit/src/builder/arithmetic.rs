use super::BlockBuilder;
use bitos::BitUtils;
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, IntCC},
};
use hemicore::arch::{InsExt, SPR, powerpc::Ins};
use tracing::{debug, debug_span};

impl BlockBuilder<'_> {
    pub fn addi(&mut self, ins: Ins) {
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, ins.field_simm() as i64);

        let value = if ins.field_ra() == 0 {
            imm
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, imm)
        };

        self.set(ins.gpr_d(), value);
    }

    pub fn addic_record(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, ins.field_simm() as i64);

        let (value, overflowed) = self.bd.ins().sadd_overflow(ra, imm);
        self.update_cr0_implicit(value, overflowed);
        self.update_xer_ca(overflowed);

        self.set(ins.gpr_d(), value);
    }

    pub fn addis(&mut self, ins: Ins) {
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, (ins.field_simm() as i64) << 16);

        let value = if ins.field_ra() == 0 {
            imm
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, imm)
        };

        self.set(ins.gpr_d(), value);
    }

    pub fn add(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());
        let (result, overflowed) = self.bd.ins().sadd_overflow(ra, rb);

        if ins.field_rc() {
            self.update_cr0_implicit(result, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.set(ins.gpr_d(), result);
    }

    pub fn subf(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());
        let (result, overflowed) = self.bd.ins().ssub_overflow(rb, ra);

        if ins.field_rc() {
            self.update_cr0_implicit(result, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.set(ins.gpr_d(), result);
    }

    pub fn ori(&mut self, ins: Ins) {
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, ins.field_uimm() as u64 as i64);
        let rs = self.get(ins.gpr_s());

        let value = self.bd.ins().bor(rs, imm);
        self.set(ins.gpr_a(), value);
    }

    pub fn oris(&mut self, ins: Ins) {
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, (ins.field_uimm() as u64 as i64) << 16);
        let rs = self.get(ins.gpr_s());

        let value = self.bd.ins().bor(rs, imm);
        self.set(ins.gpr_a(), value);
    }

    pub fn or(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let rb = self.get(ins.gpr_b());
        let value = self.bd.ins().bor(rs, rb);

        if ins.field_rc() {
            let _false = self.bd.ins().iconst(ir::types::I8, 0);
            self.update_cr0_implicit(value, _false);
        }

        self.set(ins.gpr_a(), value);
    }

    pub fn andi_record(&mut self, ins: Ins) {
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, ins.field_uimm() as u64 as i64);
        let rs = self.get(ins.gpr_s());
        let value = self.bd.ins().band(rs, imm);

        let _false = self.bd.ins().iconst(ir::types::I8, 0);
        self.update_cr0_implicit(value, _false);

        self.set(ins.gpr_a(), value);
    }

    pub fn divwu(&mut self, ins: Ins) {}

    pub fn rlwinm(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let shift_amount = ins.field_sh();
        let rotated = self.bd.ins().rotl_imm(rs, shift_amount as i64);

        let mask = if ins.field_mb() <= ins.field_me() {
            let start = 31 - ins.field_me();
            let end = 31 - ins.field_mb();
            0.with_bits(start, end + 1, !0)
        } else {
            let start = 31 - ins.field_mb();
            let end = 31 - ins.field_me();

            let mask = (!0).with_bits(start, end, 0);

            // make start exclusive too!
            mask | (1 << start)
        };

        let masked = self.bd.ins().band_imm(rotated, mask as i64);
        self.set(ins.gpr_a(), masked);
    }

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

    pub fn cmpl(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());

        self.compare_unsigned(ra, rb, ins.field_crfd());
    }

    pub fn cmpli(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, ins.field_uimm() as u64 as i64);

        self.compare_unsigned(ra, imm, ins.field_crfd());
    }
}
