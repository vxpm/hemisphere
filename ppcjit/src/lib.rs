#![feature(debug_closure_helpers)]

mod builder;
mod sequence;

pub mod block;

use crate::builder::BlockBuilder;
use cranelift::{
    codegen::{self, ir},
    frontend, native,
    prelude::Configurable,
};
use easyerr::{Error, ResultExt};
use std::sync::Arc;

pub use block::{Block, BlockFn};
pub use sequence::{Sequence, SequenceStatus};

/// A JIT compiler of [`Sequence`]s, producing [`Block`]s.
pub struct Compiler {
    isa: Arc<dyn codegen::isa::TargetIsa>,
    func_ctx: frontend::FunctionBuilderContext,
}

impl Default for Compiler {
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

impl Compiler {
    fn block_signature(&self) -> ir::Signature {
        let ptr = self.isa.pointer_type();
        ir::Signature {
            params: vec![ir::AbiParam::new(ptr); 4],
            returns: vec![ir::AbiParam::new(ir::types::I32)],
            call_conv: codegen::isa::CallConv::SystemV,
        }
    }

    pub fn compile(&mut self, sequence: Sequence) -> Result<Block, BuildError> {
        let mut func = ir::Function::new();
        func.signature = self.block_signature();

        let mut builder = BlockBuilder::new(&*self.isa, &mut func, &mut self.func_ctx);
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
