#![feature(debug_closure_helpers)]

mod builder;
mod sequence;

pub mod block;

use crate::{block::Meta, builder::BlockBuilder};
use common::arch::disasm::Ins;
use cranelift::{
    codegen::{self, ir},
    frontend, native,
    prelude::Configurable,
};
use easyerr::{Error, ResultExt};
use std::sync::Arc;

pub use block::{Block, BlockFn};
pub use sequence::Sequence;

/// A JIT compiler, producing [`Block`]s.
pub struct Compiler {
    isa: Arc<dyn codegen::isa::TargetIsa>,
    func_ctx: frontend::FunctionBuilderContext,
}

impl Default for Compiler {
    fn default() -> Self {
        let opt_level = if cfg!(debug_assertions) {
            "speed"
        } else {
            "speed_and_size"
        };

        let verifier = if cfg!(debug_assertions) {
            "true"
        } else {
            "false"
        };

        let mut settings = codegen::settings::builder();
        settings.set("use_colocated_libcalls", "false").unwrap();
        settings.set("is_pic", "false").unwrap();
        settings.set("stack_switch_model", "basic").unwrap();
        settings.set("unwind_info", "true").unwrap();
        settings.set("opt_level", opt_level).unwrap();
        settings.set("enable_verifier", verifier).unwrap();
        settings.enable("enable_alias_analysis").unwrap();
        settings.enable("enable_jump_tables").unwrap();

        let isa_builder = native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });

        let isa = isa_builder
            .finish(codegen::settings::Flags::new(settings))
            .unwrap();

        Self {
            isa,
            func_ctx: frontend::FunctionBuilderContext::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("block contains no instructions")]
    EmptyBlock,
    #[error(transparent)]
    Builder { source: builder::BuilderError },
    #[error(transparent)]
    Codegen { source: codegen::CodegenError },
}

impl Compiler {
    fn block_signature(&self) -> ir::Signature {
        let ptr = self.isa.pointer_type();
        ir::Signature {
            params: vec![ir::AbiParam::new(ptr); 4],
            returns: vec![ir::AbiParam::new(ir::types::I64)],
            call_conv: codegen::isa::CallConv::SystemV,
        }
    }

    /// Compiles a block with the given instructions (up until a terminal instruction or the end).
    pub fn compile(
        &mut self,
        instructions: impl Iterator<Item = Ins>,
    ) -> Result<Block, BuildError> {
        let mut func = ir::Function::new();
        func.signature = self.block_signature();

        let builder = BlockBuilder::new(&*self.isa, &mut func, &mut self.func_ctx);
        let (sequence, cycles) = builder.build(instructions).context(BuildCtx::Builder)?;
        if sequence.is_empty() {
            return Err(BuildError::EmptyBlock);
        }

        let mut ctx = codegen::Context::for_function(func);
        let ir = if cfg!(debug_assertions) {
            Some(ctx.func.display().to_string())
        } else {
            None
        };

        let compiled = ctx
            .compile(&*self.isa, &mut codegen::control::ControlPlane::default())
            .map_err(|e| e.inner)
            .context(BuildCtx::Codegen)?;

        let meta = Meta {
            seq: sequence,
            clir: ir,
            cycles,
        };

        let block = unsafe { Block::new(meta, &*self.isa, compiled) };
        // tracing::debug!(
        //     "compiled block:\n{}\n{}",
        //     block.meta().seq,
        //     block.meta().clir
        // );

        Ok(block)
    }
}
