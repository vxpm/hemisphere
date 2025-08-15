mod arithmetic;

use crate::Registers;
use cranelift::{
    codegen::ir::{self, condcodes::IntCC},
    frontend,
    prelude::InstBuilder,
};
use easyerr::Error;
use powerpc::{Ins, Opcode};
use std::collections::{HashMap, hash_map::Entry};

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
                offset_of!(Registers, user.gpr) + size_of::<u32>() * (i as usize)
            }
            Reg::Fpr(i) => {
                assert!(i < 32);
                offset_of!(Registers, user.fpr) + size_of::<f64>() * (i as usize)
            }
            Reg::Cr => offset_of!(Registers, user.cr),
        };

        offset as i32
    }
}

struct Var {
    inner: frontend::Variable,
    modified: bool,
}

#[derive(Debug, Error)]
pub enum EmitError {
    #[error("illegal instruction {f0:?}")]
    Illegal(Ins),
    #[error("unimplemented instruction {f0:?}")]
    Unimplemented(Ins),
}

pub struct BlockBuilder<'ctx> {
    bd: frontend::FunctionBuilder<'ctx>,
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
            bd: builder,
            regs: HashMap::new(),
            regs_ptr,
            current_bb: entry_bb,
        }
    }

    fn get(&mut self, reg: Reg) -> ir::Value {
        let var = match self.regs.entry(reg) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let dumped = self.bd.ins().load(
                    reg.ty(),
                    ir::MemFlags::trusted(),
                    self.regs_ptr,
                    reg.offset(),
                );

                let var = self.bd.declare_var(reg.ty());
                self.bd.def_var(var, dumped);
                v.insert(Var {
                    inner: var,
                    modified: false,
                })
            }
        }
        .inner;

        self.bd.use_var(var)
    }

    fn set(&mut self, reg: Reg, value: ir::Value) {
        let var = match self.regs.entry(reg) {
            Entry::Occupied(o) => {
                let var = o.into_mut();
                var.modified = true;

                var.inner
            }
            Entry::Vacant(v) => {
                let var = self.bd.declare_var(reg.ty());
                v.insert(Var {
                    inner: var,
                    modified: true,
                });

                var
            }
        };

        self.bd.def_var(var, value);
    }

    fn update_cr0(&mut self, value: ir::Value, overflowed: ir::Value) {
        let cr = self.get(Reg::Cr);

        let lt = self.bd.ins().icmp_imm(IntCC::SignedLessThan, value, 0);
        let gt = self.bd.ins().icmp_imm(IntCC::SignedGreaterThan, value, 0);
        let eq = self.bd.ins().icmp_imm(IntCC::Equal, value, 0);

        let lt = self.bd.ins().uextend(ir::types::I32, lt);
        let gt = self.bd.ins().uextend(ir::types::I32, gt);
        let eq = self.bd.ins().uextend(ir::types::I32, eq);
        let ov = self.bd.ins().uextend(ir::types::I32, overflowed);

        let lt = self.bd.ins().ishl_imm(lt, 31);
        let gt = self.bd.ins().ishl_imm(gt, 30);
        let eq = self.bd.ins().ishl_imm(eq, 29);
        let ov = self.bd.ins().ishl_imm(ov, 28);

        let value = self.bd.ins().bor(ov, eq);
        let value = self.bd.ins().bor(value, gt);
        let value = self.bd.ins().bor(value, lt);

        let mask = self.bd.ins().iconst(ir::types::I32, 0b1111 << 28);
        let updated = self.bd.ins().bitselect(mask, value, cr);

        self.set(Reg::Cr, updated);
    }

    pub fn emit(&mut self, ins: Ins) -> Result<(), EmitError> {
        match ins.op {
            Opcode::Add => self.add(ins),
            Opcode::Illegal => return Err(EmitError::Illegal(ins)),
            _ => return Err(EmitError::Unimplemented(ins)),
        }

        Ok(())
    }

    pub fn finish(mut self) {
        // emit prologue and finalize
        for (reg, var) in self.regs {
            if !var.modified {
                continue;
            }

            let value = self.bd.use_var(var.inner);
            self.bd
                .ins()
                .store(ir::MemFlags::trusted(), value, self.regs_ptr, reg.offset());
        }

        self.bd.ins().return_(&[]);
        self.bd.finalize();
    }
}
