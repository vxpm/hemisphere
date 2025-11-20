#![feature(debug_closure_helpers)]

mod builder;
mod sequence;
mod unwind;

pub mod block;

use crate::{
    block::{Meta, Trampoline},
    builder::BlockBuilder,
    unwind::UnwindHandle,
};
use cranelift::{
    codegen::{self, ir},
    frontend,
    jit::{JITBuilder, JITModule},
    module::{Linkage, Module},
    native,
    prelude::{Configurable, InstBuilder},
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
        settings.set("preserve_frame_pointers", "true").unwrap();
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
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            ..Default::default()
        }
    }

    fn block_signature(&self) -> ir::Signature {
        let ptr = self.module.isa().pointer_type();
        ir::Signature {
            params: vec![ir::AbiParam::new(ptr); 3],
            returns: vec![],
            call_conv: codegen::isa::CallConv::Tail,
        }
    }

    fn trampoline_signature(&self) -> ir::Signature {
        let ptr = self.module.isa().pointer_type();
        ir::Signature {
            params: vec![ir::AbiParam::new(ptr); 4],
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
        let hooks_ptr = params[2];
        let block_ptr = params[3];

        let block_sig = builder.import_signature(block_sig);

        builder
            .ins()
            .call_indirect(block_sig, block_ptr, &[info_ptr, ctx_ptr, hooks_ptr]);

        builder.ins().return_(&[]);
        builder.finalize();

        let mut ctx = self.module.make_context();
        ctx.func = func;

        let id = self
            .module
            .declare_anonymous_function(&ctx.func.signature)
            .unwrap();

        self.module.define_function(id, &mut ctx).unwrap();
        self.module.finalize_definitions().unwrap();

        let ptr = self.module.get_finalized_function(id);
        Trampoline(ptr)
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
            &self.settings,
            &mut func,
            &mut self.module,
            &mut self.func_ctx,
        );
        let (sequence, cycles) = builder.build(instructions).context(BuildCtx::Builder)?;
        if sequence.is_empty() {
            return Err(BuildError::EmptyBlock);
        }

        // println!("{}", func.display());

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

        let unwind_handle =
            if let Ok(Some(unwind_info)) = code.create_unwind_info(self.module.isa()) {
                unsafe { UnwindHandle::new(self.module.isa(), ptr.addr(), &unwind_info) }
            } else {
                None
            };

        // TODO: remove this and deal with handles
        std::mem::forget(unwind_handle);

        let block = Block::new(ptr, meta);
        self.count += 1;

        Ok(block)
    }
}
