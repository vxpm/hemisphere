use super::BlockBuilder;
use bitos::BitUtils;
use cranelift::{codegen::ir, prelude::InstBuilder};
use hemicore::arch::{InsExt, powerpc::Ins};
use tracing::debug;

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
            self.update_cr0(result, overflowed);
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

    pub fn rlwinm(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let shift_amount = ins.field_sh();
        let rotated = self.bd.ins().rotl_imm(rs, shift_amount as i64);

        let start = 31 - ins.field_me();
        let end = 31 - ins.field_mb();
        debug!(
            "start: {start} {} end: {end} {}",
            ins.field_mb(),
            ins.field_me()
        );

        let mask = if start > end {
            (!0).with_bits(end, start + 1, 0)
        } else {
            0.with_bits(start, end + 1, !0)
        };

        debug!("mask: {mask:032b}");

        let masked = self.bd.ins().band_imm(rotated, mask as i64);
        self.set(ins.gpr_a(), masked);
    }
}
