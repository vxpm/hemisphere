use super::BlockBuilder;
use bitos::BitUtils;
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, IntCC},
};
use hemicore::arch::{InsExt, SPR, powerpc::Ins};

impl BlockBuilder<'_> {
    pub fn ori(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let value = self.bd.ins().bor_imm(rs, ins.field_uimm() as u64 as i64);

        self.set(ins.gpr_a(), value);
    }

    pub fn oris(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let value = self
            .bd
            .ins()
            .bor_imm(rs, ((ins.field_uimm() as u64) << 16) as i64);

        self.set(ins.gpr_a(), value);
    }

    pub fn or(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let rb = self.get(ins.gpr_b());
        let value = self.bd.ins().bor(rs, rb);

        if ins.field_rc() {
            let false_ = self.bd.ins().iconst(ir::types::I8, 0);
            self.update_cr0_implicit(value, false_);
        }

        self.set(ins.gpr_a(), value);
    }

    pub fn nor(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let rb = self.get(ins.gpr_b());

        let or = self.bd.ins().bor(rs, rb);
        let nor = self.bd.ins().bnot(or);

        if ins.field_rc() {
            let false_ = self.bd.ins().iconst(ir::types::I8, 0);
            self.update_cr0_implicit(nor, false_);
        }

        self.set(ins.gpr_a(), nor);
    }

    pub fn xori(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let value = self.bd.ins().bxor_imm(rs, ins.field_uimm() as u64 as i64);

        self.set(ins.gpr_a(), value);
    }

    pub fn andi_record(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let value = self.bd.ins().band_imm(rs, ins.field_uimm() as u64 as i64);

        let false_ = self.bd.ins().iconst(ir::types::I8, 0);
        self.update_cr0_implicit(value, false_);

        self.set(ins.gpr_a(), value);
    }

    pub fn andis_record(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let value = self
            .bd
            .ins()
            .band_imm(rs, ((ins.field_uimm() as u64) << 16) as i64);

        let false_ = self.bd.ins().iconst(ir::types::I8, 0);
        self.update_cr0_implicit(value, false_);

        self.set(ins.gpr_a(), value);
    }

    pub fn and(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let rb = self.get(ins.gpr_b());
        let value = self.bd.ins().band(rs, rb);

        if ins.field_rc() {
            let false_ = self.bd.ins().iconst(ir::types::I8, 0);
            self.update_cr0_implicit(value, false_);
        }

        self.set(ins.gpr_a(), value);
    }

    pub fn andc(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let rb = self.get(ins.gpr_b());
        let not_rb = self.bd.ins().bnot(rb);
        let value = self.bd.ins().band(rs, not_rb);

        if ins.field_rc() {
            let false_ = self.bd.ins().iconst(ir::types::I8, 0);
            self.update_cr0_implicit(value, false_);
        }

        self.set(ins.gpr_a(), value);
    }

    pub fn neg(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let value = self.bd.ins().ineg(ra);
        let overflowed = self.bd.ins().icmp_imm(IntCC::Equal, ra, i32::MIN as i64);

        if ins.field_rc() {
            self.update_cr0_implicit(value, overflowed);
        }

        if ins.field_oe() {
            self.update_xer_ov(overflowed);
        }

        self.set(ins.gpr_d(), value);
    }

    pub fn rotate_left_and_mask(&mut self, ins: Ins, shift_amount: ir::Value) {
        let rs = self.get(ins.gpr_s());
        let rotated = self.bd.ins().rotl(rs, shift_amount);

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

    pub fn rlwinm(&mut self, ins: Ins) {
        let shift_amount = self
            .bd
            .ins()
            .iconst(ir::types::I32, ins.field_sh() as u64 as i64);

        self.rotate_left_and_mask(ins, shift_amount);
    }

    pub fn rlwnm(&mut self, ins: Ins) {
        let rb = self.get(ins.gpr_b());
        let shift_amount = self.bd.ins().band_imm(rb, 0x1F);

        self.rotate_left_and_mask(ins, shift_amount);
    }

    pub fn slw(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let rb = self.get(ins.gpr_b());

        let shift_by = self.bd.ins().band_imm(rb, 0x3F);

        let extended = self.bd.ins().uextend(ir::types::I64, rs);
        let shifted = self.bd.ins().ishl(extended, shift_by);
        let value = self.bd.ins().ireduce(ir::types::I32, shifted);

        if ins.field_rc() {
            let false_ = self.bd.ins().iconst(ir::types::I8, 0);
            self.update_cr0_implicit(value, false_);
        }

        self.set(ins.gpr_a(), value);
    }

    pub fn srw(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let rb = self.get(ins.gpr_b());

        let shift_by = self.bd.ins().band_imm(rb, 0x3F);

        let extended = self.bd.ins().uextend(ir::types::I64, rs);
        let shifted = self.bd.ins().ushr(extended, shift_by);
        let value = self.bd.ins().ireduce(ir::types::I32, shifted);

        if ins.field_rc() {
            let false_ = self.bd.ins().iconst(ir::types::I8, 0);
            self.update_cr0_implicit(value, false_);
        }

        self.set(ins.gpr_a(), value);
    }

    pub fn sraw(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let rb = self.get(ins.gpr_b());

        let shift_by = self.bd.ins().band_imm(rb, 0x3F);

        let extended = self.bd.ins().uextend(ir::types::I64, rs);
        let shifted = self.bd.ins().sshr(extended, shift_by);
        let value = self.bd.ins().ireduce(ir::types::I32, shifted);

        if ins.field_rc() {
            let false_ = self.bd.ins().iconst(ir::types::I8, 0);
            self.update_cr0_implicit(value, false_);
        }

        // xer ca is set if:
        // - rs is negative, and
        // - shift_by >= trailing zeros of rs
        let trailing_zeros = self.bd.ins().ctz(rs);
        let is_rs_neg = self.bd.ins().icmp_imm(IntCC::SignedLessThan, rs, 0);
        let is_shift_by_gt_tz =
            self.bd
                .ins()
                .icmp(IntCC::UnsignedGreaterThanOrEqual, shift_by, trailing_zeros);

        let carry = self.bd.ins().bor(is_rs_neg, is_shift_by_gt_tz);
        self.update_xer_ca(carry);

        self.set(ins.gpr_a(), value);
    }

    pub fn srawi(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());

        let extended = self.bd.ins().uextend(ir::types::I64, rs);
        let shifted = self
            .bd
            .ins()
            .sshr_imm(extended, ins.field_sh() as u64 as i64);
        let value = self.bd.ins().ireduce(ir::types::I32, shifted);

        if ins.field_rc() {
            let false_ = self.bd.ins().iconst(ir::types::I8, 0);
            self.update_cr0_implicit(value, false_);
        }

        // xer ca is set if:
        // - rs is negative, and
        // - shift_by >= trailing zeros of rs
        let trailing_zeros = self.bd.ins().ctz(rs);
        let is_rs_neg = self.bd.ins().icmp_imm(IntCC::SignedLessThan, rs, 0);
        let is_shift_by_gt_tz = self.bd.ins().icmp_imm(
            IntCC::UnsignedLessThan,
            trailing_zeros,
            ins.field_sh() as u64 as i64,
        );

        let carry = self.bd.ins().bor(is_rs_neg, is_shift_by_gt_tz);
        self.update_xer_ca(carry);

        self.set(ins.gpr_a(), value);
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

    pub fn cntlzw(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let value = self.bd.ins().clz(rs);

        if ins.field_rc() {
            let false_ = self.bd.ins().iconst(ir::types::I8, 0);
            self.update_cr0_implicit(value, false_);
        }

        self.set(ins.gpr_a(), value);
    }
}
