use super::BlockBuilder;
use crate::builder::{Action, InstructionInfo};
use bitos::BitUtils;
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, IntCC},
};
use gekko::{InsExt, disasm::Ins};

const LOGIC_INFO: InstructionInfo = InstructionInfo {
    cycles: 1,
    auto_pc: true,
    action: Action::Continue,
};

#[derive(Clone, Copy)]
enum BasicBitOpKind {
    Or,
    Nor,
    Xor,
    And,
    Nand,
    Eqv,
}

#[derive(Clone, Copy)]
enum BasicBitOpRhs {
    RB,
    ComplementRB,
    Imm,
    ShiftedImm,
}

#[derive(Clone, Copy)]
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

    fn basic_bitop(&mut self, ins: Ins, op: BasicBitOp) -> InstructionInfo {
        let lhs = self.get(ins.gpr_s());
        let rhs = self.basic_bitop_get_rhs(ins, op.rhs);
        let value = self.basic_bitop_compute(op.kind, lhs, rhs);

        if op.record {
            self.update_cr0_cmpz(value);
        }

        self.set(ins.gpr_a(), value);

        LOGIC_INFO
    }

    pub fn or(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::RB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn orc(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::ComplementRB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn ori(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::Imm,
                record: false,
            },
        )
    }

    pub fn oris(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Or,
                rhs: BasicBitOpRhs::ShiftedImm,
                record: false,
            },
        )
    }

    pub fn nor(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Nor,
                rhs: BasicBitOpRhs::RB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn xor(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Xor,
                rhs: BasicBitOpRhs::RB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn xori(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Xor,
                rhs: BasicBitOpRhs::Imm,
                record: false,
            },
        )
    }

    pub fn xoris(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Xor,
                rhs: BasicBitOpRhs::ShiftedImm,
                record: false,
            },
        )
    }

    pub fn and(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::RB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn andc(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::ComplementRB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn andi_record(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::Imm,
                record: true,
            },
        )
    }

    pub fn andis_record(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::And,
                rhs: BasicBitOpRhs::ShiftedImm,
                record: true,
            },
        )
    }

    pub fn nand(&mut self, ins: Ins) -> InstructionInfo {
        self.basic_bitop(
            ins,
            BasicBitOp {
                kind: BasicBitOpKind::Nand,
                rhs: BasicBitOpRhs::RB,
                record: ins.field_rc(),
            },
        )
    }

    pub fn eqv(&mut self, ins: Ins) -> InstructionInfo {
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
    fn signext(&mut self, ins: Ins, ty: ir::Type) -> InstructionInfo {
        let rs = self.get(ins.gpr_s());

        let byte = self.bd.ins().ireduce(ty, rs);
        let value = self.bd.ins().sextend(ir::types::I32, byte);

        if ins.field_rc() {
            self.update_cr0_cmpz(value);
        }

        self.set(ins.gpr_a(), value);

        LOGIC_INFO
    }

    pub fn extsb(&mut self, ins: Ins) -> InstructionInfo {
        self.signext(ins, ir::types::I8)
    }

    pub fn extsh(&mut self, ins: Ins) -> InstructionInfo {
        self.signext(ins, ir::types::I16)
    }
}

#[derive(Clone, Copy)]
enum ShiftKind {
    Left,
    RightLogic,
    RightArithmetic,
}

#[derive(Clone, Copy)]
enum ShiftRhs {
    RB,
    Imm,
}

#[derive(Clone, Copy)]
struct ShiftOp {
    kind: ShiftKind,
    rhs: ShiftRhs,
}

fn generate_mask(me: u8, mb: u8) -> u32 {
    if mb <= me {
        let start = 31 - me;
        let end = 31 - mb;
        0.with_bits(start, end + 1, !0)
    } else {
        let start = 31 - mb;
        let end = 31 - me;

        let mask = (!0).with_bits(start, end, 0);

        // make start exclusive too!
        mask | (1 << start)
    }
}

/// Rotate and Shift operations
impl BlockBuilder<'_> {
    pub fn rlwinm(&mut self, ins: Ins) -> InstructionInfo {
        let rs = self.get(ins.gpr_s());
        let mask = generate_mask(ins.field_me(), ins.field_mb());

        let rotated = self.bd.ins().rotl_imm(rs, ins.field_sh() as u64 as i64);
        let masked = self.bd.ins().band_imm(rotated, mask as i64);

        if ins.field_rc() {
            self.update_cr0_cmpz(masked);
        }

        self.set(ins.gpr_a(), masked);

        LOGIC_INFO
    }

    pub fn rlwnm(&mut self, ins: Ins) -> InstructionInfo {
        let rs = self.get(ins.gpr_s());
        let rb = self.get(ins.gpr_b());
        let mask = generate_mask(ins.field_me(), ins.field_mb());
        let shift_amount = self.bd.ins().band_imm(rb, 0x1F);

        let rotated = self.bd.ins().rotl(rs, shift_amount);
        let masked = self.bd.ins().band_imm(rotated, mask as i64);

        if ins.field_rc() {
            self.update_cr0_cmpz(masked);
        }

        self.set(ins.gpr_a(), masked);

        LOGIC_INFO
    }

    pub fn rlwimi(&mut self, ins: Ins) -> InstructionInfo {
        let rs = self.get(ins.gpr_s());
        let ra = self.get(ins.gpr_a());
        let mask = self.ir_value(generate_mask(ins.field_me(), ins.field_mb()));

        let rotated = self.bd.ins().rotl_imm(rs, ins.field_sh() as u64 as i64);
        let inserted = self.bd.ins().bitselect(mask, rotated, ra);

        if ins.field_rc() {
            self.update_cr0_cmpz(inserted);
        }

        self.set(ins.gpr_a(), inserted);

        LOGIC_INFO
    }

    fn shift_compute(&mut self, op: ShiftKind, lhs: ir::Value, rhs: ir::Value) -> ir::Value {
        match op {
            ShiftKind::Left => {
                let lhs = self.bd.ins().uextend(ir::types::I64, lhs);
                let rhs = self.bd.ins().uextend(ir::types::I64, rhs);
                let value = self.bd.ins().ishl(lhs, rhs);

                self.bd.ins().ireduce(ir::types::I32, value)
            }
            ShiftKind::RightLogic => {
                let lhs = self.bd.ins().uextend(ir::types::I64, lhs);
                let rhs = self.bd.ins().uextend(ir::types::I64, rhs);
                let value = self.bd.ins().ushr(lhs, rhs);

                self.bd.ins().ireduce(ir::types::I32, value)
            }
            ShiftKind::RightArithmetic => {
                // xer ca is set if:
                // - rs is negative, and
                // - shift_by > trailing zeros of rs
                let trailing_zeros = self.bd.ins().ctz(lhs);
                let is_rs_neg = self.bd.ins().icmp_imm(IntCC::SignedLessThan, lhs, 0);
                let is_shift_by_gt_tz =
                    self.bd
                        .ins()
                        .icmp(IntCC::UnsignedGreaterThan, rhs, trailing_zeros);

                let carry = self.bd.ins().band(is_rs_neg, is_shift_by_gt_tz);
                self.update_xer_ca(carry);

                let lhs = self.bd.ins().sextend(ir::types::I64, lhs);
                let rhs = self.bd.ins().uextend(ir::types::I64, rhs);
                let value = self.bd.ins().sshr(lhs, rhs);

                self.bd.ins().ireduce(ir::types::I32, value)
            }
        }
    }

    fn shift_get_rhs(&mut self, ins: Ins, rhs: ShiftRhs) -> ir::Value {
        match rhs {
            ShiftRhs::RB => self.get(ins.gpr_b()),
            ShiftRhs::Imm => self.ir_value(ins.field_sh() as u32),
        }
    }

    fn shift(&mut self, ins: Ins, op: ShiftOp) -> InstructionInfo {
        let lhs = self.get(ins.gpr_s());
        let rhs = self.shift_get_rhs(ins, op.rhs);

        let shift_by = self.bd.ins().band_imm(rhs, 0x3F);
        let value = self.shift_compute(op.kind, lhs, shift_by);

        if ins.field_rc() {
            self.update_cr0_cmpz(value);
        }

        self.set(ins.gpr_a(), value);

        LOGIC_INFO
    }

    pub fn slw(&mut self, ins: Ins) -> InstructionInfo {
        self.shift(
            ins,
            ShiftOp {
                kind: ShiftKind::Left,
                rhs: ShiftRhs::RB,
            },
        )
    }

    pub fn srw(&mut self, ins: Ins) -> InstructionInfo {
        self.shift(
            ins,
            ShiftOp {
                kind: ShiftKind::RightLogic,
                rhs: ShiftRhs::RB,
            },
        )
    }

    pub fn sraw(&mut self, ins: Ins) -> InstructionInfo {
        self.shift(
            ins,
            ShiftOp {
                kind: ShiftKind::RightArithmetic,
                rhs: ShiftRhs::RB,
            },
        )
    }

    pub fn srawi(&mut self, ins: Ins) -> InstructionInfo {
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
    pub fn cntlzw(&mut self, ins: Ins) -> InstructionInfo {
        let rs = self.get(ins.gpr_s());
        let value = self.bd.ins().clz(rs);

        if ins.field_rc() {
            self.update_cr0_cmpz(value);
        }

        self.set(ins.gpr_a(), value);

        LOGIC_INFO
    }
}
