#![feature(debug_closure_helpers)]

mod builder;
mod sequence;

pub mod block;
pub mod registers;

use crate::{block::Block, builder::BlockBuilder};
use cranelift::{
    codegen::{self, ir},
    frontend, native,
    prelude::Configurable,
};
use easyerr::{Error, ResultExt};
use std::sync::Arc;

pub use powerpc;
pub use registers::Registers;
pub use sequence::{Sequence, SequenceStatus};

/// A context for JIT compilation of [`Sequence`]s, producing [`Block`]s.
pub struct JIT {
    isa: Arc<dyn codegen::isa::TargetIsa>,
    func_ctx: frontend::FunctionBuilderContext,
}

impl Default for JIT {
    fn default() -> Self {
        let mut builder = codegen::settings::builder();
        builder.set("use_colocated_libcalls", "false").unwrap();
        builder.set("is_pic", "false").unwrap();
        builder.set("stack_switch_model", "basic").unwrap();
        builder.set("opt_level", "speed_and_size").unwrap();
        builder.enable("enable_alias_analysis").unwrap();
        builder.enable("enable_jump_tables").unwrap();

        let isa_builder = native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });

        let isa = isa_builder
            .finish(codegen::settings::Flags::new(builder))
            .unwrap();

        Self {
            isa,
            func_ctx: frontend::FunctionBuilderContext::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error(transparent)]
    Builder { source: builder::EmitError },
    #[error(transparent)]
    Codegen { source: codegen::CodegenError },
}

impl JIT {
    fn block_signature(&self) -> ir::Signature {
        let ptr = self.isa.pointer_type();
        ir::Signature {
            params: vec![ir::AbiParam::new(ptr), ir::AbiParam::new(ptr)],
            returns: vec![],
            call_conv: codegen::isa::CallConv::SystemV,
        }
    }

    pub fn build(&mut self, sequence: Sequence) -> Result<Block, BuildError> {
        let mut func = ir::Function::new();
        func.signature = self.block_signature();

        let mut builder = BlockBuilder::new(&mut func, &mut self.func_ctx);
        for ins in sequence.iter().copied() {
            builder.emit(ins).context(BuildCtx::Builder)?;
        }
        builder.finish();

        let mut ctx = codegen::Context::for_function(func);
        let ir = ctx.func.display().to_string();

        let compiled = ctx
            .compile(&*self.isa, &mut codegen::control::ControlPlane::default())
            .map_err(|e| e.inner)
            .context(BuildCtx::Codegen)?;

        Ok(unsafe { Block::new(sequence, ir, compiled.code_buffer()) })
    }
}

#[cfg(test)]
mod test {
    use crate::{JIT, Registers, Sequence};
    use powerpc::{Extensions, Ins};
    use powerpc_asm::{Argument, Arguments, assemble};

    #[test]
    fn test() {
        let mut seq = Sequence::new();
        let args: Arguments = [
            Argument::Unsigned(0),
            Argument::Unsigned(0),
            Argument::Unsigned(1),
            Argument::None,
            Argument::None,
        ];

        let a = assemble("add.", &args).expect("Invalid arguments");
        seq.push(Ins::new(a, Extensions::gekko_broadway())).unwrap();
        seq.push(Ins::new(a, Extensions::gekko_broadway())).unwrap();

        let mut registers = Registers::default();
        registers.user.gpr[0] = 1;
        registers.user.gpr[1] = i32::MAX as u32;

        let mut jit = JIT::default();
        let block = jit.build(seq).unwrap();
        println!("{}", block.clir());
        println!("{block}");
    }
}
