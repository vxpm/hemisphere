use std::collections::{HashMap, hash_map::Entry};

use crate::Registers;
use cranelift::{codegen::ir, frontend, prelude::InstBuilder};
use powerpc::{Ins, Opcode};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Reg {
    Gpr(u8),
    Fpr(u8),
}

impl Reg {
    #[inline]
    fn ty(self) -> ir::Type {
        match self {
            Reg::Gpr(_) => ir::types::I32,
            Reg::Fpr(_) => ir::types::F64,
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

    fn add(&mut self, ins: Ins) {
        let ra = self.get(Reg::Gpr(ins.field_ra()));
        let rb = self.get(Reg::Gpr(ins.field_rb()));
        let result = self.builder.ins().iadd(ra, rb);

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
