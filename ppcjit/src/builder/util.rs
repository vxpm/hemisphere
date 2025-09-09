use super::BlockBuilder;
use common::arch::{Reg, SPR, disasm::Ins};
use cranelift::{
    codegen::ir,
    prelude::{FunctionBuilder, InstBuilder, IntCC},
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

impl BlockBuilder<'_> {
    /// Stub instruction - does absolutely nothing.
    pub fn stub(&mut self, _: Ins) {
        self.bd.ins().nop();
    }

    /// Creates an IR value from the given `value`.
    pub fn ir_value(&mut self, value: impl IntoIrValue) -> ir::Value {
        value.into_value(&mut self.bd)
    }

    /// Gets bit `index` in the `value` (must be an I32).
    pub fn get_bit(&mut self, value: ir::Value, index: impl IntoIrValue) -> ir::Value {
        let one = self.ir_value(1i32);
        let index = self.ir_value(index);

        let mask = self.bd.ins().ishl(one, index);
        let masked = self.bd.ins().band(value, mask);
        let bit = self.bd.ins().ushr(masked, index);

        self.bd.ins().ireduce(ir::types::I8, bit)
    }

    /// Sets bit `index` to `set` in the `value` (must be an I32).
    pub fn set_bit(
        &mut self,
        value: ir::Value,
        index: impl IntoIrValue,
        set: impl IntoIrValue,
    ) -> ir::Value {
        let zero = self.ir_value(0i32);
        let one = self.ir_value(1i32);
        let index = self.ir_value(index);
        let set = self.ir_value(set);

        // create mask for the bit
        let shifted = self.bd.ins().ishl(one, index);
        let mask = self.bd.ins().bnot(shifted);

        // unset bit
        let value = self.bd.ins().band(value, mask);

        // set bit if `set` is true
        let rhs = self.bd.ins().select(set, shifted, zero);

        self.bd.ins().bor(value, rhs)
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
        let ov = self.bd.ins().ishl_imm(ov, base + 0);

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
        let ov = self.get_bit(xer, 30);

        self.update_cr(0, lt, gt, eq, ov);
    }

    /// Updates CR0 by signed comparison of the given value with 0 and withe the given overflow
    /// flag. Value must be an I32.
    pub fn update_cr0_cmpz_ov(&mut self, value: ir::Value, ov: ir::Value) {
        let lt = self.bd.ins().icmp_imm(IntCC::SignedLessThan, value, 0);
        let gt = self.bd.ins().icmp_imm(IntCC::SignedGreaterThan, value, 0);
        let eq = self.bd.ins().icmp_imm(IntCC::Equal, value, 0);

        self.update_cr(0, lt, gt, eq, ov);
    }
}
