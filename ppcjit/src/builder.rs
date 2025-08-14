use crate::{Context, Registers};
use cranelift::{codegen::ir, frontend, prelude::InstBuilder};
use powerpc::{Ins, Opcode};

struct Variables {
    gpr: [frontend::Variable; 32],
    fpr: [frontend::Variable; 32],
}

pub struct BlockBuilder<'ctx> {
    builder: frontend::FunctionBuilder<'ctx>,
    vars: Variables,
    current_bb: ir::Block,
}

impl<'ctx> BlockBuilder<'ctx> {
    fn prelude(regs_ptr: ir::Value, builder: &mut frontend::FunctionBuilder<'ctx>) -> Variables {
        let mut gpr = Vec::with_capacity(32);
        let mut fpr = Vec::with_capacity(32);
        for i in 0..32 {
            let dumped = builder.ins().load(
                ir::types::I32,
                ir::MemFlags::trusted(),
                regs_ptr,
                std::mem::offset_of!(Registers, gpr) as i32 + 4 * i,
            );

            let var = builder.declare_var(ir::types::I32);
            builder.def_var(var, dumped);
            gpr.push(var);
        }

        for i in 0..32 {
            let dumped = builder.ins().load(
                ir::types::F32,
                ir::MemFlags::trusted(),
                regs_ptr,
                std::mem::offset_of!(Registers, fpr) as i32 + 4 * i,
            );

            let var = builder.declare_var(ir::types::F32);
            builder.def_var(var, dumped);
            fpr.push(var);
        }

        Variables {
            gpr: gpr.try_into().unwrap(),
            fpr: fpr.try_into().unwrap(),
        }
    }

    pub fn new(ctx: &'ctx mut Context) -> Self {
        let mut builder = frontend::FunctionBuilder::new(&mut ctx.codegen.func, &mut ctx.func);
        let entry_bb = builder.create_block();
        builder.append_block_params_for_function_params(entry_bb);
        builder.switch_to_block(entry_bb);
        builder.seal_block(entry_bb);

        // emit prelude: dump registers
        let regs_ptr = builder.block_params(entry_bb)[0];
        let vars = Self::prelude(regs_ptr, &mut builder);

        Self {
            builder,
            vars,
            current_bb: entry_bb,
        }
    }

    fn add(&mut self, ins: Ins) {
        let ra = self.builder.use_var(self.vars.gpr[ins.field_ra() as usize]);
        let rb = self.builder.use_var(self.vars.gpr[ins.field_rb() as usize]);
        let result = self.builder.ins().iadd(ra, rb);

        self.builder
            .def_var(self.vars.gpr[ins.field_rd() as usize], result);
    }

    pub fn emit(&mut self, ins: Ins) {
        match ins.op {
            Opcode::Add => self.add(ins),
            Opcode::Illegal => panic!("illegal opcode"),
            _ => todo!("unimplemented opcode"),
        }
    }

    pub fn finish(mut self) {
        self.builder.ins().return_(&[]);
        self.builder.finalize();
    }
}
