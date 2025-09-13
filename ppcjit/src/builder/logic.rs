use super::BlockBuilder;
use crate::builder::{Info, util::IntoIrValue};
use bitos::BitUtils;
use common::arch::{InsExt, disasm::Ins};
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, IntCC},
};

const LOGIC_INFO: Info = Info {
    cycles: 1,
    auto_pc: true,
};

enum BasicBitOpKind {
    Or,
    Nor,
    Xor,
    And,
    Nand,
    Eqv,
}

enum BasicBitOpRhs {
    RB,
    ComplementRB,
    Imm,
    ShiftedImm,
}

struct BasicBitOp {
    kind: BasicBitOpKind,
    rhs: BasicBitOpRhs,
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
            BasicBitOpRhs::RB => self.get(ins.gpr_b()),
            BasicBitOpRhs::ComplementRB => {
                let rb = self.get(ins.gpr_b());
                self.bd.ins().bnot(rb)
            }
            BasicBitOpRhs::Imm => self.ir_value(ins.field_uimm() as u32),
            BasicBitOpRhs::ShiftedImm => self.ir_value((ins.field_uimm() as u32) << 16),
        }
    }

    fn basic_bitop(&mut self, ins: Ins, op: BasicBitOp) -> Info {
        let lhs = self.get(ins.gpr_s());
        let rhs = self.basic_bitop_get_rhs(ins, op.rhs);
        let value = self.basic_bitop_compute(op.kind, lhs, rhs);

        if op.record {
            self.update_cr0_cmpz(value);
        }

        self.set(ins.gpr_a(), value);

        LOGIC_INFO
    }

    pub fn or(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::RB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn orc(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::ComplementRB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn ori(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::Imm,
                record: false,
            },
        )
    }

    pub fn oris(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::ShiftedImm,
                record: false,
            },
        )
    }

    pub fn nor(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Nor,
                rhs: BasicBitOpRhs::RB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn xor(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Xor,
                rhs: BasicBitOpRhs::RB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn xori(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Xor,
                rhs: BasicBitOpRhs::Imm,
                record: false,
            },
        )
    }

    pub fn and(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::RB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn andc(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::ComplementRB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn andi_record(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::Imm,
                record: true,
            },
        )
    }

    pub fn andis_record(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::ShiftedImm,
                record: true,
            },
        )
    }

    pub fn nand(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Nand,
                rhs: BasicBitOpRhs::RB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn eqv(&mut self, ins: Ins) -> Info {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Eqv,
                rhs: BasicBitOpRhs::RB,
                record: ins.field_rc(),
            },
        )
    }
}

/// Signed extension operations
impl BlockBuilder<'_> {
    fn signext(&mut self, ins: Ins, ty: ir::Type) -> Info {
        let rs = self.get(ins.gpr_s());

        let byte = self.bd.ins().ireduce(ty, rs);
        let value = self.bd.ins().sextend(ir::types::I32, byte);

        if ins.field_rc() {
            self.update_cr0_cmpz(value);
        }

        self.set(ins.gpr_a(), value);

        LOGIC_INFO
    }

    pub fn extsb(&mut self, ins: Ins) -> Info {
        self.signext(ins, ir::types::I8)
    }

    pub fn extsh(&mut self, ins: Ins) -> Info {
        self.signext(ins, ir::types::I16)
    }
}

enum ShiftKind {
    Left,
    RightLogic,
    RightArithmetic,
}

enum ShiftRhs {
    RB,
    Imm,
}

struct ShiftOp {
    kind: ShiftKind,
    rhs: ShiftRhs,
}

/// Rotate and Shift operations
impl BlockBuilder<'_> {
    fn generate_mask(&mut self, ins: Ins) -> i32 {
        if ins.field_mb() <= ins.field_me() {
            let start = 31 - ins.field_me();
            let end = 31 - ins.field_mb();
            0.with_bits(start, end + 1, !0)
        } else {
            let start = 31 - ins.field_mb();
            let end = 31 - ins.field_me();

            let mask = (!0).with_bits(start, end, 0);

            // make start exclusive too!
            mask | (1 << start)
        }
    }

    pub fn rlwinm(&mut self, ins: Ins) -> Info {
        let rs = self.get(ins.gpr_s());
        let mask = self.generate_mask(ins);

        let rotated = self.bd.ins().rotl_imm(rs, ins.field_sh() as u64 as i64);
        let masked = self.bd.ins().band_imm(rotated, mask as i64);

        self.set(ins.gpr_a(), masked);

        LOGIC_INFO
    }

    pub fn rlwnm(&mut self, ins: Ins) -> Info {
        let rs = self.get(ins.gpr_s());
        let rb = self.get(ins.gpr_b());
        let mask = self.generate_mask(ins);
        let shift_amount = self.bd.ins().band_imm(rb, 0x1F);

        let rotated = self.bd.ins().rotl(rs, shift_amount);
        let masked = self.bd.ins().band_imm(rotated, mask as i64);

        self.set(ins.gpr_a(), masked);

        LOGIC_INFO
    }

    pub fn rlwimi(&mut self, ins: Ins) -> Info {
        let rs = self.get(ins.gpr_s());
        let ra = self.get(ins.gpr_a());
        let mask = self.generate_mask(ins).into_value(&mut self.bd);

        let rotated = self.bd.ins().rotl_imm(rs, ins.field_sh() as u64 as i64);
        let inserted = self.bd.ins().bitselect(mask, rotated, ra);

        self.set(ins.gpr_a(), inserted);

        LOGIC_INFO
    }

    fn shift_compute(&mut self, op: ShiftKind, lhs: ir::Value, rhs: ir::Value) -> ir::Value {
        match op {
            ShiftKind::Left => self.bd.ins().ishl(lhs, rhs),
            ShiftKind::RightLogic => self.bd.ins().ushr(lhs, rhs),
            ShiftKind::RightArithmetic => {
                // xer ca is set if:
                // - rs is negative, and
                // - shift_by >= trailing zeros of rs
                let trailing_zeros = self.bd.ins().ctz(lhs);
                let trailing_zeros = self.bd.ins().ireduce(ir::types::I32, trailing_zeros);

                let is_rs_neg = self.bd.ins().icmp_imm(IntCC::SignedLessThan, lhs, 0);
                let is_shift_by_gt_tz =
                    self.bd
                        .ins()
                        .icmp(IntCC::UnsignedGreaterThanOrEqual, rhs, trailing_zeros);

                let carry = self.bd.ins().bor(is_rs_neg, is_shift_by_gt_tz);
                self.update_xer_ca(carry);

                self.bd.ins().sshr(lhs, rhs)
            }
        }
    }

    fn shift_get_rhs(&mut self, ins: Ins, rhs: ShiftRhs) -> ir::Value {
        match rhs {
            ShiftRhs::RB => self.get(ins.gpr_b()),
            ShiftRhs::Imm => self.ir_value(ins.field_sh() as u32),
        }
    }

    fn shift(&mut self, ins: Ins, op: ShiftOp) -> Info {
        let lhs = self.get(ins.gpr_s());
        let rhs = self.shift_get_rhs(ins, op.rhs);

        let shift_by = self.bd.ins().band_imm(rhs, 0x3F);
        let extended = self.bd.ins().uextend(ir::types::I64, lhs);
        let shifted = self.shift_compute(op.kind, extended, shift_by);
        let value = self.bd.ins().ireduce(ir::types::I32, shifted);

        if ins.field_rc() {
            self.update_cr0_cmpz(value);
        }

        self.set(ins.gpr_a(), value);

        LOGIC_INFO
    }

    pub fn slw(&mut self, ins: Ins) -> Info {
        self.shift(
            ins,
            ShiftOp {
                kind: ShiftKind::Left,
                rhs: ShiftRhs::RB,
            },
        )
    }

    pub fn srw(&mut self, ins: Ins) -> Info {
        self.shift(
            ins,
            ShiftOp {
                kind: ShiftKind::RightLogic,
                rhs: ShiftRhs::RB,
            },
        )
    }

    pub fn sraw(&mut self, ins: Ins) -> Info {
        self.shift(
            ins,
            ShiftOp {
                kind: ShiftKind::RightArithmetic,
                rhs: ShiftRhs::RB,
            },
        )
    }

    pub fn srawi(&mut self, ins: Ins) -> Info {
        self.shift(
            ins,
            ShiftOp {
                kind: ShiftKind::RightArithmetic,
                rhs: ShiftRhs::Imm,
            },
        )
    }
}

/// Misc operations
impl BlockBuilder<'_> {
    pub fn cntlzw(&mut self, ins: Ins) -> Info {
        let rs = self.get(ins.gpr_s());
        let value = self.bd.ins().clz(rs);

        if ins.field_rc() {
            self.update_cr0_cmpz(value);
        }

        self.set(ins.gpr_a(), value);

        LOGIC_INFO
    }
}
