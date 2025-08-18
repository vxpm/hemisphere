mod arithmetic;
mod branch;
mod memory;
mod others;

use crate::{Registers, block::BlockOutput};
use cranelift::{
    codegen::ir::{self, condcodes::IntCC},
    frontend,
    prelude::{InstBuilder, isa::TargetIsa},
};
use easyerr::Error;
use num_enum::TryFromPrimitive;
use powerpc::{Ins, Opcode};
use std::collections::{HashMap, hash_map::Entry};
use std::mem::offset_of;

#[derive(Clone, Copy, PartialEq, Eq, Hash, TryFromPrimitive)]
#[repr(u16)]
enum Spr {
    XER = 1,
    LR = 8,
    CTR = 9,
}

impl Spr {
    fn offset(&self) -> i32 {
        let offset = match self {
            Self::XER => offset_of!(Registers, user.xer),
            Self::LR => offset_of!(Registers, user.lr),
            Self::CTR => offset_of!(Registers, user.ctr),
        };

        offset as i32
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[expect(dead_code, reason = "still not used")]
enum Reg {
    Gpr(u8),
    Fpr(u8),
    Spr(Spr),
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
            Reg::Spr(spr) => return spr.offset(),
        };

        offset as i32
    }
}

struct RegState {
    var: frontend::Variable,
    modified: bool,
}

#[derive(Debug, Error)]
pub enum EmitError {
    #[error("illegal instruction {f0:?}")]
    Illegal(Ins),
    #[error("unimplemented instruction {f0:?}")]
    Unimplemented(Ins),
}

struct Context {
    ptr_type: ir::Type,
    regs_ptr: ir::Value,
    external_data_ptr: ir::Value,
    external_functions_ptr: ir::Value,
    output_ptr: ir::Value,
}

pub struct BlockBuilder<'ctx> {
    bd: frontend::FunctionBuilder<'ctx>,
    ctx: Context,
    regs: HashMap<Reg, RegState>,
    current_bb: ir::Block,
    executed: u32,
}

impl<'ctx> BlockBuilder<'ctx> {
    pub fn new(
        isa: &'ctx dyn TargetIsa,
        func: &'ctx mut ir::Function,
        ctx: &'ctx mut frontend::FunctionBuilderContext,
    ) -> Self {
        let mut builder = frontend::FunctionBuilder::new(func, ctx);
        let entry_bb = builder.create_block();
        builder.append_block_params_for_function_params(entry_bb);
        builder.switch_to_block(entry_bb);
        builder.seal_block(entry_bb);

        let params = builder.block_params(entry_bb);
        let ctx = Context {
            ptr_type: isa.pointer_type(),
            regs_ptr: params[0],
            external_data_ptr: params[1],
            external_functions_ptr: params[2],
            output_ptr: params[3],
        };

        Self {
            bd: builder,
            ctx,
            regs: HashMap::new(),
            current_bb: entry_bb,
            executed: 0,
        }
    }

    fn get(&mut self, reg: Reg) -> ir::Value {
        let var = match self.regs.entry(reg) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let dumped = self.bd.ins().load(
                    reg.ty(),
                    ir::MemFlags::trusted(),
                    self.ctx.regs_ptr,
                    reg.offset(),
                );

                let var = self.bd.declare_var(reg.ty());
                self.bd.def_var(var, dumped);
                v.insert(RegState {
                    var,
                    modified: false,
                })
            }
        }
        .var;

        self.bd.use_var(var)
    }

    fn set(&mut self, reg: Reg, value: ir::Value) {
        let var = match self.regs.entry(reg) {
            Entry::Occupied(o) => {
                let var = o.into_mut();
                var.modified = true;

                var.var
            }
            Entry::Vacant(v) => {
                let var = self.bd.declare_var(reg.ty());
                v.insert(RegState {
                    var,
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

        let value = self.bd.ins().bor(lt, gt);
        let value = self.bd.ins().bor(value, eq);
        let value = self.bd.ins().bor(value, ov);

        let mask = self.bd.ins().iconst(ir::types::I32, 0b1111 << 28);
        let updated = self.bd.ins().bitselect(mask, value, cr);

        self.set(Reg::Cr, updated);
    }

    pub fn emit(&mut self, ins: Ins) -> Result<(), EmitError> {
        match ins.op {
            Opcode::Add => self.add(ins),
            Opcode::Addis => self.addis(ins),
            Opcode::Ori => self.ori(ins),
            Opcode::B => self.branch(ins),
            Opcode::Mfspr => self.mfspr(ins),
            Opcode::Stwu => self.stwu(ins),
            Opcode::Illegal => return Err(EmitError::Illegal(ins)),
            _ => return Err(EmitError::Unimplemented(ins)),
        }

        self.executed += 1;

        Ok(())
    }

    pub fn finish(mut self) {
        for (reg, var) in self.regs {
            if !var.modified {
                continue;
            }

            let value = self.bd.use_var(var.var);
            self.bd.ins().store(
                ir::MemFlags::trusted(),
                value,
                self.ctx.regs_ptr,
                reg.offset(),
            );
        }

        let executed = self
            .bd
            .ins()
            .iconst(ir::types::I32, self.executed as u64 as i64);
        self.bd.ins().store(
            ir::MemFlags::trusted(),
            executed,
            self.ctx.output_ptr,
            offset_of!(BlockOutput, executed) as i32,
        );

        self.bd.ins().return_(&[]);
        self.bd.finalize();
    }
}
