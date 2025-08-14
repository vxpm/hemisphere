use std::collections::{HashMap, hash_map::Entry};

use crate::Registers;
use cranelift::{
    codegen::ir::{self, condcodes::IntCC},
    frontend,
    prelude::InstBuilder,
};
use powerpc::{Ins, Opcode};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Reg {
    Gpr(u8),
    Fpr(u8),
    Cr,
}

impl Reg {
    #[inline]
    fn ty(self) -> ir::Type {
        match self {
            Reg::Fpr(_) => ir::types::F64,
            _ => ir::types::I32,
        }
    }

    #[inline]
    fn offset(self) -> i32 {
        use std::mem::offset_of;
        let offset = match self {
            Reg::Gpr(i) => {
                assert!(i < 32);
                offset_of!(Registers, gpr) + size_of::<u32>() * (i as usize)
            }
            Reg::Fpr(i) => {
                assert!(i < 32);
                offset_of!(Registers, fpr) + size_of::<f64>() * (i as usize)
            }
            Reg::Cr => offset_of!(Registers, cr),
        };

        offset as i32
    }
}

struct Var {
    inner: frontend::Variable,
    modified: bool,
}

pub struct BlockBuilder<'ctx> {
    builder: frontend::FunctionBuilder<'ctx>,
    regs: HashMap<Reg, Var>,
    regs_ptr: ir::Value,
    current_bb: ir::Block,
}

impl<'ctx> BlockBuilder<'ctx> {
    pub fn new(
        func: &'ctx mut ir::Function,
        ctx: &'ctx mut frontend::FunctionBuilderContext,
    ) -> Self {
        let mut builder = frontend::FunctionBuilder::new(func, ctx);
        let entry_bb = builder.create_block();
        builder.append_block_params_for_function_params(entry_bb);
        builder.switch_to_block(entry_bb);
        builder.seal_block(entry_bb);

        let regs_ptr = builder.block_params(entry_bb)[0];

        Self {
            builder,
            regs: HashMap::new(),
            regs_ptr,
            current_bb: entry_bb,
        }
    }

    fn get(&mut self, reg: Reg) -> ir::Value {
        let var = match self.regs.entry(reg) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let dumped = self.builder.ins().load(
                    reg.ty(),
                    ir::MemFlags::trusted(),
                    self.regs_ptr,
                    reg.offset(),
                );

                let var = self.builder.declare_var(reg.ty());
                self.builder.def_var(var, dumped);
                v.insert(Var {
                    inner: var,
                    modified: false,
                })
            }
        }
        .inner;

        self.builder.use_var(var)
    }

    fn set(&mut self, reg: Reg, value: ir::Value) {
        let var = match self.regs.entry(reg) {
            Entry::Occupied(o) => {
                let var = o.into_mut();
                var.modified = true;

                var.inner
            }
            Entry::Vacant(v) => {
                let var = self.builder.declare_var(reg.ty());
                v.insert(Var {
                    inner: var,
                    modified: true,
                });

                var
            }
        };

        self.builder.def_var(var, value);
    }

    /// `value` must be I32
    fn set_bit(&mut self, value: ir::Value, index: u8, bit: ir::Value) -> ir::Value {
        let bit = self.builder.ins().uextend(ir::types::I32, bit);
        let one = self.builder.ins().iconst(ir::types::I32, 1);
        let bit = self.builder.ins().band(bit, one);
        let shift_amount = self.builder.ins().iconst(ir::types::I32, index as i64);
        let shifted = self.builder.ins().ishl(one, shift_amount);
        let mask = self.builder.ins().bnot(shifted);

        let masked = self.builder.ins().band(value, mask);
        self.builder.ins().bor(masked, bit)
    }

    fn update_cr0(&mut self, value: ir::Value, overflowed: ir::Value) {
        let zero = self.builder.ins().iconst(ir::types::I32, 0);
        let cr = self.get(Reg::Cr);

        let lt = self.builder.ins().icmp(IntCC::SignedLessThan, value, zero);
        let gt = self
            .builder
            .ins()
            .icmp(IntCC::SignedGreaterThan, value, zero);
        let eq = self.builder.ins().icmp(IntCC::Equal, value, zero);

        let updated = self.set_bit(cr, 0, lt);
        let updated = self.set_bit(updated, 1, gt);
        let updated = self.set_bit(updated, 2, eq);
        let updated = self.set_bit(updated, 3, overflowed);

        self.set(Reg::Cr, updated);
    }

    fn add(&mut self, ins: Ins) {
        let ra = self.get(Reg::Gpr(ins.field_ra()));
        let rb = self.get(Reg::Gpr(ins.field_rb()));
        let result = self.builder.ins().iadd(ra, rb);

        if true {
            // if ins.field_rc() {
            let min = self.builder.ins().iconst(ir::types::I32, i32::MIN as i64);
            let max = self.builder.ins().iconst(ir::types::I32, i32::MAX as i64);
            let zero = self.builder.ins().iconst(ir::types::I32, 0);
            let ra_gte_zero = self
                .builder
                .ins()
                .icmp(IntCC::SignedGreaterThanOrEqual, ra, zero);
            let not_ra_gte_zero = self.builder.ins().bnot(ra_gte_zero);

            let upper_limit = self.builder.ins().isub(max, ra);
            let lower_limit = self.builder.ins().isub(min, ra);
            let b_gt_upper = self
                .builder
                .ins()
                .icmp(IntCC::SignedGreaterThan, rb, upper_limit);
            let b_lt_lower = self
                .builder
                .ins()
                .icmp(IntCC::SignedLessThan, rb, lower_limit);

            let cond_a = self.builder.ins().band(ra_gte_zero, b_gt_upper);
            let cond_b = self.builder.ins().band(not_ra_gte_zero, b_lt_lower);

            let overflowed = self.builder.ins().bor(cond_a, cond_b);
            self.update_cr0(result, overflowed);
        }

        self.set(Reg::Gpr(ins.field_rd()), result);
    }

    pub fn emit(&mut self, ins: Ins) {
        match ins.op {
            Opcode::Add => self.add(ins),
            Opcode::Illegal => panic!("illegal opcode"),
            _ => todo!("unimplemented opcode"),
        }
    }

    pub fn finish(mut self) {
        // emit prologue and finalize
        for (reg, var) in self.regs {
            if !var.modified {
                continue;
            }

            let value = self.builder.use_var(var.inner);
            self.builder
                .ins()
                .store(ir::MemFlags::trusted(), value, self.regs_ptr, reg.offset());
        }

        self.builder.ins().return_(&[]);
        self.builder.finalize();
    }
}
