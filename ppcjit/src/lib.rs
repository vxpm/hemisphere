#![feature(debug_closure_helpers)]

mod builder;
mod sequence;

pub mod block;

use crate::{block::Meta, builder::BlockBuilder};
use cranelift::{
    codegen::{self, ir},
    frontend,
    jit::{JITBuilder, JITModule},
    module::{Linkage, Module},
    native,
    prelude::Configurable,
};
use easyerr::{Error, ResultExt};
use gekko::disasm::Ins;

pub use block::{Block, BlockFn};
pub use sequence::Sequence;

#[derive(Debug, Clone)]
pub struct Settings {
    /// Whether to treat `sc` instructions as no-ops.
    pub nop_syscalls: bool,
    /// Whether to ignore the FPU enabled bit in MSR.
    pub force_fpu: bool,
    /// Whether to ignore unimplemented instructions instead of panicking.
    pub ignore_unimplemented: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            nop_syscalls: false,
            force_fpu: false,
            ignore_unimplemented: false,
        }
    }
}

/// A JIT compiler, producing [`Block`]s.
pub struct Compiler {
    settings: Settings,
    module: JITModule,
    func_ctx: frontend::FunctionBuilderContext,
    count: u64,
}

impl Default for Compiler {
    fn default() -> Self {
        let opt_level = "speed_and_size";
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

        let isa_builder = native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });

        let isa = isa_builder
            .finish(codegen::settings::Flags::new(settings))
            .unwrap();

        let module = JITModule::new(JITBuilder::with_isa(
            isa,
            cranelift::module::default_libcall_names(),
        ));

        Self {
            module,
            settings: Default::default(),
            func_ctx: frontend::FunctionBuilderContext::new(),
            count: 0,
        }
    }
}

impl Compiler {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            ..Default::default()
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
        let ptr = self.module.isa().pointer_type();
        ir::Signature {
            params: vec![ir::AbiParam::new(ptr); 2],
            returns: vec![ir::AbiParam::new(ir::types::I64)],
            call_conv: codegen::isa::CallConv::SystemV,
        }
    }

    /// Compiles a block with the given instructions (up until a terminal instruction or the end of
    /// the iterator).
    pub fn compile(
        &mut self,
        instructions: impl Iterator<Item = Ins>,
    ) -> Result<Block, BuildError> {
        let mut func = ir::Function::new();
        func.signature = self.block_signature();

        let builder = BlockBuilder::new(
            self.module.isa(),
            &self.settings,
            &mut func,
            &mut self.func_ctx,
        );
        let (sequence, cycles) = builder.build(instructions).context(BuildCtx::Builder)?;
        if sequence.is_empty() {
            return Err(BuildError::EmptyBlock);
        }

        let ir = cfg!(debug_assertions).then(|| func.display().to_string());
        let meta = Meta {
            idle_loop: sequence.detect_idle_loop(),
            clir: ir,
            cycles,
            seq: sequence,
        };

        let mut ctx = self.module.make_context();
        ctx.func = func;

        let id = self
            .module
            .declare_function(
                &format!("block_{}", self.count),
                Linkage::Export,
                &ctx.func.signature,
            )
            .unwrap();

        self.module.define_function(id, &mut ctx).unwrap();
        self.module.finalize_definitions().unwrap();

        let ptr = self.module.get_finalized_function(id);
        let code = ctx.compiled_code().unwrap();
        let block = Block::new(ptr, code, meta, self.module.isa());

        self.count += 1;

        Ok(block)
    }
}
