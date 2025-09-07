use super::BlockBuilder;
use bitos::BitUtils;
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, IntCC},
};
use hemicore::arch::{InsExt, powerpc::Ins};

pub enum BasicBitOpKind {
    Or,
    Nor,
    Xor,
    And,
    Nand,
    Eqv,
}

pub enum BasicBitOpRhs {
    GPRB,
    ComplementGPRB,
    Imm,
    ShiftedImm,
}

pub struct BasicBitOp {
    /// Operation to perform
    kind: BasicBitOpKind,
    /// What value to use as the second operand
    rhs: BasicBitOpRhs,
    /// Whether to update CR0
    record: bool,
}

/// Basic bit operations
impl BlockBuilder<'_> {
    fn basic_bitop_compute(
        &mut self,
        op: BasicBitOpKind,
        lhs: ir::Value,
        rhs: ir::Value,
    ) -> ir::Value {
        match op {
            BasicBitOpKind::Or => self.bd.ins().bor(lhs, rhs),
            BasicBitOpKind::Nor => {
                let or = self.bd.ins().bor(lhs, rhs);
                self.bd.ins().bnot(or)
            }
            BasicBitOpKind::Xor => self.bd.ins().bxor(lhs, rhs),
            BasicBitOpKind::And => self.bd.ins().band(lhs, rhs),
            BasicBitOpKind::Nand => {
                let and = self.bd.ins().band(lhs, rhs);
                self.bd.ins().bnot(and)
            }
            BasicBitOpKind::Eqv => {
                let xor = self.bd.ins().bxor(lhs, rhs);
                self.bd.ins().bnot(xor)
            }
        }
    }

    fn basic_bitop_get_rhs(&mut self, ins: Ins, rhs: BasicBitOpRhs) -> ir::Value {
        match rhs {
            BasicBitOpRhs::GPRB => self.get(ins.gpr_b()),
            BasicBitOpRhs::ComplementGPRB => {
                let rb = self.get(ins.gpr_b());
                self.bd.ins().bnot(rb)
            }
            BasicBitOpRhs::Imm => self
                .bd
                .ins()
                .iconst(ir::types::I32, ins.field_uimm() as u64 as i64),
            BasicBitOpRhs::ShiftedImm => self
                .bd
                .ins()
                .iconst(ir::types::I32, ((ins.field_uimm() as u64) << 16) as i64),
        }
    }

    pub fn basic_bitop(&mut self, ins: Ins, op: BasicBitOp) {
        let lhs = self.get(ins.gpr_s());
        let rhs = self.basic_bitop_get_rhs(ins, op.rhs);
        let value = self.basic_bitop_compute(op.kind, lhs, rhs);

        if op.record {
            let false_ = self.false_const();
            self.update_cr0_cmpz(value, false_);
        }

        self.set(ins.gpr_a(), value);
    }

    pub fn or(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::GPRB,
                record: ins.field_rc(),
            },
        );
    }

    pub fn orc(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::ComplementGPRB,
                record: ins.field_rc(),
            },
        );
    }

    pub fn ori(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::Imm,
                record: false,
            },
        );
    }

    pub fn oris(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::ShiftedImm,
                record: false,
            },
        );
    }

    pub fn nor(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Nor,
                rhs: BasicBitOpRhs::GPRB,
                record: ins.field_rc(),
            },
        );
    }

    pub fn xor(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Xor,
                rhs: BasicBitOpRhs::GPRB,
                record: ins.field_rc(),
            },
        );
    }

    pub fn xori(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Xor,
                rhs: BasicBitOpRhs::Imm,
                record: false,
            },
        );
    }

    pub fn and(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::GPRB,
                record: ins.field_rc(),
            },
        );
    }

    pub fn andc(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::ComplementGPRB,
                record: ins.field_rc(),
            },
        );
    }

    pub fn andi_record(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::Imm,
                record: true,
            },
        );
    }

    pub fn andis_record(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::ShiftedImm,
                record: true,
            },
        );
    }

    pub fn nand(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Nand,
                rhs: BasicBitOpRhs::GPRB,
                record: ins.field_rc(),
            },
        );
    }

    pub fn eqv(&mut self, ins: Ins) {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Eqv,
                rhs: BasicBitOpRhs::GPRB,
                record: ins.field_rc(),
            },
        );
    }
}

/// Signed extension operations
impl BlockBuilder<'_> {
    fn signext(&mut self, ins: Ins, ty: ir::Type) {
        let rs = self.get(ins.gpr_s());

        let byte = self.bd.ins().ireduce(ty, rs);
        let value = self.bd.ins().sextend(ir::types::I32, byte);

        if ins.field_rc() {
            let false_ = self.false_const();
            self.update_cr0_cmpz(value, false_);
        }

        self.set(ins.gpr_a(), value);
    }

    pub fn extsb(&mut self, ins: Ins) {
        self.signext(ins, ir::types::I8);
    }

    pub fn extsh(&mut self, ins: Ins) {
        self.signext(ins, ir::types::I16);
    }
}

/// Rotate and Shift operations
impl BlockBuilder<'_> {
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
            let false_ = self.false_const();
            self.update_cr0_cmpz(value, false_);
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
            let false_ = self.false_const();
            self.update_cr0_cmpz(value, false_);
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
            let false_ = self.false_const();
            self.update_cr0_cmpz(value, false_);
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
            let false_ = self.false_const();
            self.update_cr0_cmpz(value, false_);
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
}

/// Misc operations
impl BlockBuilder<'_> {
    pub fn cntlzw(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let value = self.bd.ins().clz(rs);

        if ins.field_rc() {
            let false_ = self.false_const();
            self.update_cr0_cmpz(value, false_);
        }

        self.set(ins.gpr_a(), value);
    }
}
