use super::BlockBuilder;
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, IntCC},
};
use hemicore::arch::{InsExt, SPR, powerpc::Ins};

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

    pub fn addic(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, ins.field_simm() as i64);

        let (value, overflowed) = self.bd.ins().sadd_overflow(ra, imm);
        self.update_xer_ca(overflowed);

        self.set(ins.gpr_d(), value);
    }

    pub fn addic_record(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, ins.field_simm() as i64);

        let (value, overflowed) = self.bd.ins().sadd_overflow(ra, imm);
        self.update_cr0_cmpz(value, overflowed);
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
            self.update_cr0_cmpz(result, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.set(ins.gpr_d(), result);
    }

    pub fn addze(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());

        let xer = self.get(SPR::XER);
        let shifted = self.bd.ins().ushr_imm(xer, 29);
        let ca = self.bd.ins().band_imm(shifted, 1);

        let (result, overflowed) = self.bd.ins().sadd_overflow(ra, ca);

        if ins.field_rc() {
            self.update_cr0_cmpz(result, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.update_xer_ca(overflowed);
        self.set(ins.gpr_d(), result);
    }

    pub fn addc(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());
        let (result, overflowed) = self.bd.ins().sadd_overflow(ra, rb);

        if ins.field_rc() {
            self.update_cr0_cmpz(result, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.update_xer_ca(overflowed);
        self.set(ins.gpr_d(), result);
    }

    pub fn adde(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());

        let xer = self.get(SPR::XER);
        let shifted = self.bd.ins().ushr_imm(xer, 29);
        let ca = self.bd.ins().band_imm(shifted, 1);

        let (result, overflowed_a) = self.bd.ins().sadd_overflow(ra, rb);
        let (result, overflowed_b) = self.bd.ins().sadd_overflow(result, ca);
        let overflowed = self.bd.ins().bor(overflowed_a, overflowed_b);

        if ins.field_rc() {
            self.update_cr0_cmpz(result, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.update_xer_ca(overflowed);
        self.set(ins.gpr_d(), result);
    }

    pub fn subf(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());
        let (result, overflowed) = self.bd.ins().ssub_overflow(rb, ra);

        if ins.field_rc() {
            self.update_cr0_cmpz(result, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.set(ins.gpr_d(), result);
    }

    pub fn subfe(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());

        let xer = self.get(SPR::XER);
        let shifted = self.bd.ins().ushr_imm(xer, 29);
        let ca = self.bd.ins().band_imm(shifted, 1);

        // workaround ssub_overflow_bin not compiling (cranelift bug?)
        let not_ra = self.bd.ins().bnot(ra);
        let sub_val = self.bd.ins().iadd(not_ra, ca);
        let (result, overflowed) = self.bd.ins().ssub_overflow(rb, sub_val);

        if ins.field_rc() {
            self.update_cr0_cmpz(result, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.update_xer_ca(overflowed);
        self.set(ins.gpr_d(), result);
    }

    pub fn subfc(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());
        let (result, overflowed) = self.bd.ins().ssub_overflow(rb, ra);

        if ins.field_rc() {
            self.update_cr0_cmpz(result, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.update_xer_ca(overflowed);
        self.set(ins.gpr_d(), result);
    }

    pub fn subfic(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, ins.field_simm() as i64);

        let (result, overflowed) = self.bd.ins().ssub_overflow(imm, ra);

        self.update_xer_ca(overflowed);
        self.set(ins.gpr_d(), result);
    }

    pub fn neg(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let value = self.bd.ins().ineg(ra);
        let overflowed = self.bd.ins().icmp_imm(IntCC::Equal, ra, i32::MIN as i64);

        if ins.field_rc() {
            self.update_cr0_cmpz(value, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.set(ins.gpr_d(), value);
    }

    pub fn divwu(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());

        let one = self.bd.ins().iconst(ir::types::I32, 1);
        let div_by_zero = self.bd.ins().icmp_imm(IntCC::Equal, rb, 0);
        let denom = self.bd.ins().select(div_by_zero, one, rb);

        let result = self.bd.ins().udiv(ra, denom);

        if ins.field_rc() {
            self.update_cr0_cmpz(result, div_by_zero);
        }

        if ins.field_oe() {
            self.update_xer_ov(div_by_zero);
        }

        self.set(ins.gpr_d(), result);
    }

    pub fn mullw(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());

        let (result, overflowed) = self.bd.ins().smul_overflow(ra, rb);

        if ins.field_rc() {
            self.update_cr0_cmpz(result, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.set(ins.gpr_d(), result);
    }

    pub fn mulli(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, ins.field_simm() as i64);

        let result = self.bd.ins().imul(ra, imm);
        self.set(ins.gpr_d(), result);
    }

    pub fn mulhwu(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());

        let result = self.bd.ins().smulhi(ra, rb);

        if ins.field_rc() {
            let false_ = self.bd.ins().iconst(ir::types::I8, 0);
            self.update_cr0_cmpz(result, false_);
        }

        self.set(ins.gpr_d(), result);
    }
}
