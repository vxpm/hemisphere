#![feature(debug_closure_helpers)]

mod allocator;
mod builder;
mod module;
mod sequence;
mod unwind;

pub mod block;
pub mod hooks;

use std::sync::Arc;

use crate::{
    block::{Meta, Trampoline},
    builder::BlockBuilder,
    hooks::Hooks,
    module::Module,
    unwind::UnwindHandle,
};
use cranelift::{
    codegen::{self, ir},
    frontend, native,
    prelude::{Configurable, InstBuilder, isa::TargetIsa},
};
use easyerr::{Error, ResultExt};
use gekko::disasm::Ins;

pub use block::Block;
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

struct Compiler {
    settings: Settings,
    hooks: Hooks,
    isa: Arc<dyn TargetIsa>,
    module: Module,
}

impl Compiler {
    fn new(settings: Settings, hooks: Hooks) -> Self {
        let opt_level = "speed_and_size";
        let verifier = if cfg!(debug_assertions) {
            "true"
        } else {
            "false"
        };

        let mut codegen = codegen::settings::builder();
        codegen.set("preserve_frame_pointers", "true").unwrap();
        codegen.set("use_colocated_libcalls", "false").unwrap();
        codegen.set("is_pic", "false").unwrap();
        codegen.set("stack_switch_model", "basic").unwrap();
        codegen.set("unwind_info", "true").unwrap();
        codegen.set("opt_level", opt_level).unwrap();
        codegen.set("enable_verifier", verifier).unwrap();
        codegen.enable("enable_alias_analysis").unwrap();

        let isa_builder = native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });

        let isa = isa_builder
            .finish(codegen::settings::Flags::new(codegen))
            .unwrap();

        Compiler {
            settings,
            hooks,
            isa,
            module: Module::new(),
        }
    }
}

/// A JIT compiler, producing [`Block`]s.
pub struct JIT {
    compiler: Compiler,
    code_ctx: codegen::Context,
    func_ctx: frontend::FunctionBuilderContext,
    compiled_count: u64,
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

impl JIT {
    pub fn new(settings: Settings, hooks: Hooks) -> Self {
        Self {
            compiler: Compiler::new(settings, hooks),
            code_ctx: codegen::Context::new(),
            func_ctx: frontend::FunctionBuilderContext::new(),
            compiled_count: 0,
        }
    }

    fn block_signature(&self) -> ir::Signature {
        let ptr = self.compiler.isa.pointer_type();
        ir::Signature {
            params: vec![ir::AbiParam::new(ptr); 2],
            returns: vec![],
            call_conv: codegen::isa::CallConv::Tail,
        }
    }

    fn trampoline_signature(&self) -> ir::Signature {
        let ptr = self.compiler.isa.pointer_type();
        ir::Signature {
            params: vec![ir::AbiParam::new(ptr); 3],
            returns: vec![],
            call_conv: codegen::isa::CallConv::SystemV,
        }
    }

    /// Compiles and returns a trampoline to call blocks.
    pub fn trampoline(&mut self) -> Trampoline {
        let block_sig = self.block_signature();

        let mut func = ir::Function::new();
        func.signature = self.trampoline_signature();

        let mut builder = frontend::FunctionBuilder::new(&mut func, &mut self.func_ctx);
        let entry_bb = builder.create_block();
        builder.append_block_params_for_function_params(entry_bb);
        builder.switch_to_block(entry_bb);
        builder.seal_block(entry_bb);

        let params = builder.block_params(entry_bb);
        let info_ptr = params[0];
        let ctx_ptr = params[1];
        let block_ptr = params[2];

        let block_sig = builder.import_signature(block_sig);
        builder
            .ins()
            .call_indirect(block_sig, block_ptr, &[info_ptr, ctx_ptr]);

        builder.ins().return_(&[]);
        builder.finalize();

        self.code_ctx.clear();
        self.code_ctx.func = func;
        self.code_ctx
            .compile(&*self.compiler.isa, &mut Default::default())
            .unwrap();

        let compiled = self.code_ctx.take_compiled_code().unwrap();
        let alloc = self.compiler.module.allocate_code(compiled.code_buffer());

        Trampoline(alloc)
    }

    /// Compiles a block with the given instructions (up until a terminal instruction or the end of
    /// the iterator).
    pub fn compile(
        &mut self,
        instructions: impl Iterator<Item = Ins>,
    ) -> Result<Block, BuildError> {
        let mut func = ir::Function::new();
        func.signature = self.block_signature();

        let func_builder = frontend::FunctionBuilder::new(&mut func, &mut self.func_ctx);
        let builder = BlockBuilder::new(&mut self.compiler, func_builder);

        let (sequence, cycles) = builder.build(instructions).context(BuildCtx::Builder)?;
        if sequence.is_empty() {
            return Err(BuildError::EmptyBlock);
        }

        // println!("{}", func.display());

        // let ir = cfg!(debug_assertions).then(|| func.display().to_string());
        let ir = func.display().to_string();
        let meta = Meta {
            pattern: sequence.detect_idle_loop(),
            clir: Some(ir),
            cycles,
            seq: sequence,
        };

        self.code_ctx.clear();
        self.code_ctx.func = func;
        self.code_ctx
            .compile(&*self.compiler.isa, &mut Default::default())
            .unwrap();

        let compiled = self.code_ctx.compiled_code().unwrap();
        let alloc = self.compiler.module.allocate_code(compiled.code_buffer());

        let unwind_handle = if let Ok(Some(unwind_info)) =
            compiled.create_unwind_info(&*self.compiler.isa)
        {
            unsafe { UnwindHandle::new(&*self.compiler.isa, alloc.as_ptr().addr(), &unwind_info) }
        } else {
            None
        };

        // TODO: remove this and deal with handles
        std::mem::forget(unwind_handle);

        let block = Block::new(alloc, meta);
        self.compiled_count += 1;

        Ok(block)
    }
}
