use super::BlockBuilder;
use crate::builder::util::IntoIrValue;
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, IntCC},
};
use hemicore::arch::{InsExt, SPR, powerpc::Ins};

enum AddLhs {
    RA,
    ZeroOrRA,
}

enum AddRhs {
    RB,
    Imm,
    ShiftedImm,
    Carry,
}

struct AddOp {
    lhs: AddLhs,
    rhs: AddRhs,
    extend: bool,
    record: bool,
    overflow: bool,
    carry: bool,
}

/// Add operations
impl BlockBuilder<'_> {
    fn addition_get_lhs(&mut self, ins: Ins, lhs: AddLhs) -> ir::Value {
        match lhs {
            AddLhs::RA => self.get(ins.gpr_a()),
            AddLhs::ZeroOrRA => {
                if ins.field_ra() == 0 {
                    0i32.into_value(&mut self.bd)
                } else {
                    self.get(ins.gpr_a())
                }
            }
        }
    }

    fn addition_get_rhs(&mut self, ins: Ins, rhs: AddRhs) -> ir::Value {
        match rhs {
            AddRhs::RB => self.get(ins.gpr_b()),
            AddRhs::Imm => self
                .bd
                .ins()
                .iconst(ir::types::I32, ins.field_simm() as i64),
            AddRhs::ShiftedImm => self
                .bd
                .ins()
                .iconst(ir::types::I32, (ins.field_simm() as i64) << 16),
            AddRhs::Carry => {
                let xer = self.get(SPR::XER);
                let ca = self.get_bit(xer, 29);
                self.bd.ins().uextend(ir::types::I32, ca)
            }
        }
    }

    fn addition_compute(
        &mut self,
        lhs: ir::Value,
        rhs: ir::Value,
        cin: ir::Value,
    ) -> (ir::Value, ir::Value, ir::Value) {
        let (value, cout_a) = self.bd.ins().uadd_overflow(lhs, rhs);
        let (value, cout_b) = self.bd.ins().uadd_overflow(value, cin);
        let cout = self.bd.ins().bor(cout_a, cout_b);

        let lhs_sign = self.bd.ins().band_imm(lhs, 0b1 << 31);
        let rhs_sign = self.bd.ins().band_imm(rhs, 0b1 << 31);
        let value_sign = self.bd.ins().band_imm(value, 0b1 << 31);

        let lhs_eq_rhs = self.bd.ins().icmp(IntCC::Equal, lhs_sign, rhs_sign);
        let value_sign_diff = self.bd.ins().icmp(IntCC::NotEqual, value_sign, lhs);
        let overflow = self.bd.ins().band(lhs_eq_rhs, value_sign_diff);

        (value, cout, overflow)
    }

    fn addition(&mut self, ins: Ins, op: AddOp) {
        let lhs = self.addition_get_lhs(ins, op.lhs);
        let rhs = self.addition_get_rhs(ins, op.rhs);

        let cin = if op.extend {
            let xer = self.get(SPR::XER);
            let ca = self.get_bit(xer, 29);
            self.bd.ins().uextend(ir::types::I32, ca)
        } else {
            0i32.into_value(&mut self.bd)
        };

        let (value, cout, overflowed) = self.addition_compute(lhs, rhs, cin);

        if op.record {
            self.update_cr0_cmpz(value);
        }

        if op.carry {
            self.update_xer_ca(cout);
        }

        if op.overflow {
            self.update_xer_ov(overflowed);
        }

        self.set(ins.gpr_d(), value);
    }

    pub fn add(&mut self, ins: Ins) {
        self.addition(
            ins,
            AddOp {
                lhs: AddLhs::RA,
                rhs: AddRhs::RB,
                extend: false,
                record: ins.field_rc(),
                carry: false,
                overflow: ins.field_oe(),
            },
        );
    }

    pub fn addc(&mut self, ins: Ins) {
        self.addition(
            ins,
            AddOp {
                lhs: AddLhs::RA,
                rhs: AddRhs::RB,
                extend: false,
                record: ins.field_rc(),
                carry: true,
                overflow: ins.field_oe(),
            },
        );
    }

    pub fn adde(&mut self, ins: Ins) {
        self.addition(
            ins,
            AddOp {
                lhs: AddLhs::RA,
                rhs: AddRhs::RB,
                extend: true,
                record: ins.field_rc(),
                carry: true,
                overflow: ins.field_oe(),
            },
        );
    }

    pub fn addze(&mut self, ins: Ins) {
        self.addition(
            ins,
            AddOp {
                lhs: AddLhs::RA,
                rhs: AddRhs::Carry,
                extend: true,
                record: ins.field_rc(),
                carry: true,
                overflow: ins.field_oe(),
            },
        );
    }

    pub fn addi(&mut self, ins: Ins) {
        self.addition(
            ins,
            AddOp {
                lhs: AddLhs::ZeroOrRA,
                rhs: AddRhs::Imm,
                extend: false,
                record: false,
                carry: false,
                overflow: false,
            },
        );
    }

    pub fn addis(&mut self, ins: Ins) {
        self.addition(
            ins,
            AddOp {
                lhs: AddLhs::ZeroOrRA,
                rhs: AddRhs::ShiftedImm,
                extend: false,
                record: false,
                carry: false,
                overflow: false,
            },
        );
    }

    pub fn addic(&mut self, ins: Ins) {
        self.addition(
            ins,
            AddOp {
                lhs: AddLhs::RA,
                rhs: AddRhs::Imm,
                extend: false,
                record: false,
                carry: true,
                overflow: false,
            },
        );
    }

    pub fn addic_record(&mut self, ins: Ins) {
        self.addition(
            ins,
            AddOp {
                lhs: AddLhs::RA,
                rhs: AddRhs::Imm,
                extend: false,
                record: true,
                carry: true,
                overflow: false,
            },
        );
    }
}

impl BlockBuilder<'_> {
    pub fn subf(&mut self, ins: Ins) {
        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());
        let (result, overflowed) = self.bd.ins().ssub_overflow(rb, ra);

        if ins.field_rc() {
            self.update_cr0_cmpz(result);
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
            self.update_cr0_cmpz(result);
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
            self.update_cr0_cmpz(result);
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
            self.update_cr0_cmpz(value);
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
            self.update_cr0_cmpz_ov(result, div_by_zero);
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
            self.update_cr0_cmpz_ov(result, overflowed);
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
            self.update_cr0_cmpz(result);
        }

        self.set(ins.gpr_d(), result);
    }
}
