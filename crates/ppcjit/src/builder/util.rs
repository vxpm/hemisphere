use super::{Action, BlockBuilder};
use crate::builder::InstructionInfo;
use cranelift::{
    codegen::ir,
    prelude::{FloatCC, FunctionBuilder, InstBuilder, IntCC},
};
use gekko::{
    Reg, SPR,
    disasm::{Ins, ParsedIns},
};

/// Trait for transforming values into an IR value in a function.
pub trait IntoIrValue {
    fn into_value(self, bd: &mut FunctionBuilder<'_>) -> ir::Value;
}

impl IntoIrValue for ir::Value {
    fn into_value(self, _: &mut FunctionBuilder<'_>) -> ir::Value {
        self
    }
}

impl IntoIrValue for bool {
    fn into_value(self, bd: &mut FunctionBuilder<'_>) -> ir::Value {
        bd.ins().iconst(ir::types::I8, self as i64)
    }
}

impl IntoIrValue for i8 {
    fn into_value(self, bd: &mut FunctionBuilder<'_>) -> ir::Value {
        bd.ins().iconst(ir::types::I8, self as i64)
    }
}

impl IntoIrValue for u8 {
    fn into_value(self, bd: &mut FunctionBuilder<'_>) -> ir::Value {
        bd.ins().iconst(ir::types::I8, self as u64 as i64)
    }
}

impl IntoIrValue for i16 {
    fn into_value(self, bd: &mut FunctionBuilder<'_>) -> ir::Value {
        bd.ins().iconst(ir::types::I16, self as i64)
    }
}

impl IntoIrValue for u16 {
    fn into_value(self, bd: &mut FunctionBuilder<'_>) -> ir::Value {
        bd.ins().iconst(ir::types::I16, self as u64 as i64)
    }
}

impl IntoIrValue for i32 {
    fn into_value(self, bd: &mut FunctionBuilder<'_>) -> ir::Value {
        bd.ins().iconst(ir::types::I32, self as i64)
    }
}

impl IntoIrValue for u32 {
    fn into_value(self, bd: &mut FunctionBuilder<'_>) -> ir::Value {
        bd.ins().iconst(ir::types::I32, self as u64 as i64)
    }
}

impl IntoIrValue for f32 {
    fn into_value(self, bd: &mut FunctionBuilder<'_>) -> ir::Value {
        bd.ins().f32const(self)
    }
}

impl IntoIrValue for f64 {
    fn into_value(self, bd: &mut FunctionBuilder<'_>) -> ir::Value {
        bd.ins().f64const(self)
    }
}

impl BlockBuilder<'_> {
    /// NOP instruction - does absolutely nothing on purpose.
    pub fn nop(&mut self, action: Action) -> InstructionInfo {
        self.bd.ins().nop();
        InstructionInfo {
            cycles: 2,
            auto_pc: true,
            action,
        }
    }

    /// Stub instruction - does absolutely nothing as a temporary implementation.
    #[allow(dead_code)]
    pub fn stub(&mut self, ins: Ins) -> InstructionInfo {
        let mut parsed = ParsedIns::new();
        ins.parse_basic(&mut parsed);

        tracing::warn!("emitting stubbed instruction ({parsed})");

        self.bd.ins().nop();
        InstructionInfo {
            cycles: 2,
            auto_pc: true,
            action: Action::FlushAndPrologue,
        }
    }

    /// Creates an IR value from the given `value`.
    pub fn ir_value(&mut self, value: impl IntoIrValue) -> ir::Value {
        value.into_value(&mut self.bd)
    }

    /// Gets bit `index` in the `value` (must be an I32).
    pub fn get_bit(&mut self, value: ir::Value, index: impl IntoIrValue) -> ir::Value {
        let index = self.ir_value(index);

        let shifted = self.bd.ins().ushr(value, index);
        let bit = self.bd.ins().band_imm(shifted, 0b1);

        self.bd.ins().ireduce(ir::types::I8, bit)
    }

    /// Sets bit `index` to `set` in the `value` (must be an I32).
    pub fn set_bit(
        &mut self,
        value: ir::Value,
        index: impl IntoIrValue,
        should_set: impl IntoIrValue,
    ) -> ir::Value {
        let zero = self.ir_value(0i32);
        let one = self.ir_value(1i32);
        let index = self.ir_value(index);
        let should_set = self.ir_value(should_set);

        // create mask for the bit
        let mask = self.bd.ins().ishl(one, index);

        // unset bit
        let value = self.bd.ins().band_not(value, mask);

        // set bit if `should_set` is true
        let rhs = self.bd.ins().select(should_set, mask, zero);

        self.bd.ins().bor(value, rhs)
    }

    pub fn round_to_single(&mut self, value: ir::Value) -> ir::Value {
        let single = self.bd.ins().fdemote(ir::types::F32, value);
        self.bd.ins().fpromote(ir::types::F64, single)
    }

    /// Updates OV and SO in XER. `overflowed` must be a boolean (I8).
    pub fn update_xer_ov(&mut self, overflowed: impl IntoIrValue) {
        let xer = self.get(SPR::XER);
        let overflowed = self.ir_value(overflowed);
        let overflowed = self.bd.ins().uextend(ir::types::I32, overflowed);

        let ov = self.bd.ins().ishl_imm(overflowed, 30);
        let so = self.bd.ins().ishl_imm(overflowed, 31);
        let value = self.bd.ins().bor(ov, so);

        let mask = self.ir_value(!(0b1 << 30));
        let masked = self.bd.ins().band(xer, mask);
        let updated = self.bd.ins().bor(masked, value);

        self.set(SPR::XER, updated);
    }

    /// Updates CA in XER. `carry` must be a boolean (I8).
    pub fn update_xer_ca(&mut self, carry: impl IntoIrValue) {
        let xer = self.get(SPR::XER);
        let updated = self.set_bit(xer, 29, carry);

        self.set(SPR::XER, updated);
    }

    /// All IR values must be booleans (i.e. I8).
    pub fn update_cr(
        &mut self,
        index: u8,
        lt: ir::Value,
        gt: ir::Value,
        eq: ir::Value,
        ov: ir::Value,
    ) {
        let cr = self.get(Reg::CR);

        let lt = self.bd.ins().uextend(ir::types::I32, lt);
        let gt = self.bd.ins().uextend(ir::types::I32, gt);
        let eq = self.bd.ins().uextend(ir::types::I32, eq);
        let ov = self.bd.ins().uextend(ir::types::I32, ov);

        let base = (4 * (7 - index)) as u64 as i64;
        let lt = self.bd.ins().ishl_imm(lt, base + 3);
        let gt = self.bd.ins().ishl_imm(gt, base + 2);
        let eq = self.bd.ins().ishl_imm(eq, base + 1);
        let ov = self.bd.ins().ishl_imm(ov, base);

        let value = self.bd.ins().bor(lt, gt);
        let value = self.bd.ins().bor(value, eq);
        let value = self.bd.ins().bor(value, ov);

        let mask = self.ir_value(0b1111u32 << base);
        let updated = self.bd.ins().bitselect(mask, value, cr);

        self.set(Reg::CR, updated);
    }

    /// Updates CR0 by signed comparison of the given value with 0 and by copying the overflow flag
    /// from XER SO. Value must be an I32.
    pub fn update_cr0_cmpz(&mut self, value: ir::Value) {
        let lt = self.bd.ins().icmp_imm(IntCC::SignedLessThan, value, 0);
        let gt = self.bd.ins().icmp_imm(IntCC::SignedGreaterThan, value, 0);
        let eq = self.bd.ins().icmp_imm(IntCC::Equal, value, 0);

        let xer = self.get(SPR::XER);
        let ov = self.get_bit(xer, 31);

        self.update_cr(0, lt, gt, eq, ov);
    }

    ///// Updates CR0 by signed comparison of the given value with 0 and withe the given overflow
    ///// flag. Value must be an I32.
    //pub fn update_cr0_cmpz_ov(&mut self, value: ir::Value, ov: ir::Value) {
    //    let lt = self.bd.ins().icmp_imm(IntCC::SignedLessThan, value, 0);
    //    let gt = self.bd.ins().icmp_imm(IntCC::SignedGreaterThan, value, 0);
    //    let eq = self.bd.ins().icmp_imm(IntCC::Equal, value, 0);
    //
    //    self.update_cr(0, lt, gt, eq, ov);
    //}

    /// All IR values must be booleans (i.e. I8).
    pub fn update_fprf(&mut self, lt: ir::Value, gt: ir::Value, eq: ir::Value, un: ir::Value) {
        let fpscr = self.get(Reg::FPSCR);

        let lt = self.bd.ins().uextend(ir::types::I32, lt);
        let gt = self.bd.ins().uextend(ir::types::I32, gt);
        let eq = self.bd.ins().uextend(ir::types::I32, eq);
        let un = self.bd.ins().uextend(ir::types::I32, un);

        let cc = self.ir_value(0);
        let lt = self.bd.ins().ishl_imm(lt, 15);
        let gt = self.bd.ins().ishl_imm(gt, 14);
        let eq = self.bd.ins().ishl_imm(eq, 13);
        let un = self.bd.ins().ishl_imm(un, 12);

        let value = self.bd.ins().bor(lt, gt);
        let value = self.bd.ins().bor(value, eq);
        let value = self.bd.ins().bor(value, un);
        let value = self.bd.ins().bor(value, cc);

        let mask = self.ir_value(0b11111u32 << 12);
        let updated = self.bd.ins().bitselect(mask, value, fpscr);

        self.set(Reg::FPSCR, updated);
    }

    pub fn update_fprf_cmpz(&mut self, value: ir::Value) {
        let zero = self.ir_value(0.0f64);
        let lt = self.bd.ins().fcmp(FloatCC::LessThan, value, zero);
        let gt = self.bd.ins().fcmp(FloatCC::GreaterThan, value, zero);
        let eq = self.bd.ins().fcmp(FloatCC::Equal, value, zero);
        let un = self.bd.ins().fcmp(FloatCC::Unordered, value, zero);
        self.update_fprf(lt, gt, eq, un);
    }

    pub fn update_fpscr(&mut self) {
        tracing::warn!("update FEX and VX")
    }

    /// Updates CR1 by copying bits 28..32 of FPSCR.
    pub fn update_cr1_float(&mut self) {
        self.update_fpscr();

        let fpscr = self.get(Reg::FPSCR);
        let cr = self.get(Reg::CR);

        let bits = self.bd.ins().ushr_imm(fpscr, 4);
        let mask = self.ir_value(0b1111 << 24);
        let updated = self.bd.ins().bitselect(mask, bits, cr);

        self.set(Reg::CR, updated);
    }
}
