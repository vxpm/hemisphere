#![feature(debug_closure_helpers)]

mod builder;
mod module;
mod sequence;
mod unwind;

pub mod block;
pub mod hooks;

use crate::{
    block::{BlockFn, Info, Meta, Trampoline},
    builder::BlockBuilder,
    hooks::{Context, Hooks},
    module::Module,
    unwind::UnwindHandle,
};
use clif_incremental::RedbCache;
use cranelift::{
    codegen::{self, ir},
    frontend, native,
    prelude::{Configurable, InstBuilder, isa::TargetIsa},
};
use easyerr::{Error, ResultExt};
use gekko::disasm::Ins;
use std::{ptr::NonNull, sync::Arc};

pub use block::Block;
pub use sequence::Sequence;

#[derive(Debug, Clone, Default)]
pub struct Settings {
    /// Whether to treat `sc` instructions as no-ops.
    pub nop_syscalls: bool,
    /// Whether to ignore the FPU enabled bit in MSR.
    pub force_fpu: bool,
    /// Whether to ignore unimplemented instructions instead of panicking.
    pub ignore_unimplemented: bool,
}

pub const FASTMEM_LUT_COUNT: usize = 1 << 15;
pub type FastmemLut = [Option<NonNull<u8>>; FASTMEM_LUT_COUNT];

// fn empty_fastmem() -> NonNull<FastmemLut> {
//     #[derive(Clone, Copy)]
//     #[repr(transparent)]
//     struct SendSyncWrapper(*mut u8);
//     unsafe impl Send for SendSyncWrapper {}
//     unsafe impl Sync for SendSyncWrapper {}
//
//     static EMPTY_FASTMEM_LUT: [SendSyncWrapper; FASTMEM_LUT_COUNT] =
//         [SendSyncWrapper(std::ptr::null_mut()); FASTMEM_LUT_COUNT];
//
//     NonNull::new((&raw const EMPTY_FASTMEM_LUT).cast_mut().cast()).unwrap()
// }

struct Compiler {
    settings: Settings,
    hooks: Hooks,
    isa: Arc<dyn TargetIsa>,
    module: Module,
}

impl Compiler {
    fn new(settings: Settings, hooks: Hooks) -> Self {
        let verifier = if cfg!(debug_assertions) {
            "true"
        } else {
            "false"
        };

        let mut codegen = codegen::settings::builder();
        codegen.set("preserve_frame_pointers", "true").unwrap();
        codegen.set("use_colocated_libcalls", "false").unwrap();
        codegen.set("stack_switch_model", "basic").unwrap();
        codegen.set("unwind_info", "true").unwrap();
        codegen.set("is_pic", "false").unwrap();

        // affect runtime performance
        codegen.set("opt_level", "speed").unwrap();
        codegen.set("enable_verifier", verifier).unwrap();
        codegen.set("enable_alias_analysis", "true").unwrap();
        codegen.set("regalloc_algorithm", "backtracking").unwrap();
        codegen.set("regalloc_checker", "false").unwrap();
        codegen.set("enable_pinned_reg", "false").unwrap();
        codegen
            .set("enable_heap_access_spectre_mitigation", "false")
            .unwrap();
        codegen
            .set("enable_table_access_spectre_mitigation", "false")
            .unwrap();

        let isa_builder = native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });

        let flags = codegen::settings::Flags::new(codegen);
        let isa = isa_builder.finish(flags).unwrap();

        Compiler {
            settings,
            hooks,
            isa,
            module: Module::new(),
        }
    }

    fn block_signature(&self) -> ir::Signature {
        let ptr = self.isa.pointer_type();
        ir::Signature {
            // info, ctx, regs, fastmem
            params: vec![ir::AbiParam::new(ptr); 4],
            returns: vec![],
            call_conv: codegen::isa::CallConv::Tail,
        }
    }

    fn trampoline_signature(&self) -> ir::Signature {
        let ptr = self.isa.pointer_type();
        ir::Signature {
            params: vec![ir::AbiParam::new(ptr); 3],
            returns: vec![],
            call_conv: codegen::isa::CallConv::SystemV,
        }
    }

    /// Compiles and returns a trampoline to call blocks.
    fn trampoline(
        &mut self,
        code_ctx: &mut codegen::Context,
        func_ctx: &mut frontend::FunctionBuilderContext,
    ) -> Trampoline {
        let block_sig = self.block_signature();

        let mut func = ir::Function::new();
        func.signature = self.trampoline_signature();

        let mut builder = frontend::FunctionBuilder::new(&mut func, func_ctx);
        let entry_bb = builder.create_block();
        builder.append_block_params_for_function_params(entry_bb);
        builder.switch_to_block(entry_bb);
        builder.seal_block(entry_bb);

        let params = builder.block_params(entry_bb);
        let info_ptr = params[0];
        let ctx_ptr = params[1];
        let block_ptr = params[2];
        let ptr_type = self.isa.pointer_type();

        // extract regs ptr
        let get_regs_sig = builder.import_signature(Hooks::get_registers_sig(ptr_type));
        let get_registers = builder
            .ins()
            .iconst(ptr_type, self.hooks.get_registers as usize as i64);
        let inst = builder
            .ins()
            .call_indirect(get_regs_sig, get_registers, &[ctx_ptr]);
        let regs_ptr = builder.inst_results(inst)[0];

        // extract fastmem ptr
        let get_fmem_sig = builder.import_signature(Hooks::get_fastmem_sig(ptr_type));
        let get_fmem = builder
            .ins()
            .iconst(ptr_type, self.hooks.get_fastmem as usize as i64);
        let inst = builder
            .ins()
            .call_indirect(get_fmem_sig, get_fmem, &[ctx_ptr]);
        let fmem_ptr = builder.inst_results(inst)[0];

        // call the block
        let block_sig = builder.import_signature(block_sig);
        builder.ins().call_indirect(
            block_sig,
            block_ptr,
            &[info_ptr, ctx_ptr, regs_ptr, fmem_ptr],
        );

        builder.ins().return_(&[]);
        builder.finalize();

        code_ctx.clear();
        code_ctx.func = func;
        code_ctx
            .compile(&*self.isa, &mut Default::default())
            .unwrap();

        let compiled = code_ctx.take_compiled_code().unwrap();
        let alloc = self.module.allocate_code(compiled.code_buffer());

        Trampoline(alloc)
    }
}

/// A JIT context, producing [`Block`]s.
pub struct Jit {
    compiler: Compiler,
    code_ctx: codegen::Context,
    func_ctx: frontend::FunctionBuilderContext,
    cache: RedbCache,
    compiled_count: u64,
    trampoline: Trampoline,
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

impl Jit {
    pub fn new(settings: Settings, hooks: Hooks) -> Self {
        let mut compiler = Compiler::new(settings, hooks);
        let mut code_ctx = codegen::Context::new();
        let mut func_ctx = frontend::FunctionBuilderContext::new();

        let trampoline = compiler.trampoline(&mut code_ctx, &mut func_ctx);
        let cache = RedbCache::new("ppcjit", false);

        Self {
            compiler,
            code_ctx,
            func_ctx,
            cache,
            compiled_count: 0,
            trampoline,
        }
    }

    /// Compiles a block with the given instructions (up until a terminal instruction or the end of
    /// the iterator).
    pub fn compile(
        &mut self,
        instructions: impl Iterator<Item = Ins>,
    ) -> Result<Block, BuildError> {
        let mut func = ir::Function::new();
        func.signature = self.compiler.block_signature();

        let func_builder = frontend::FunctionBuilder::new(&mut func, &mut self.func_ctx);
        let builder = BlockBuilder::new(&mut self.compiler, func_builder);

        let (sequence, cycles) = builder.build(instructions).context(BuildCtx::Builder)?;
        if sequence.is_empty() {
            return Err(BuildError::EmptyBlock);
        }

        // println!("{}", func.display());

        // let ir = func.display().to_string();
        let ir = cfg!(debug_assertions).then(|| func.display().to_string());
        let meta = Meta {
            pattern: sequence.detect_idle_loop(),
            clir: ir,
            cycles,
            seq: sequence,
        };

        self.code_ctx.clear();
        self.code_ctx.func = func;
        self.code_ctx
            .compile_with_cache(
                &*self.compiler.isa,
                &mut self.cache,
                &mut Default::default(),
            )
            .unwrap();

        let compiled = self.code_ctx.compiled_code().unwrap();
        let alloc = self.compiler.module.allocate_code(compiled.code_buffer());

        let unwind_handle =
            if let Ok(Some(unwind_info)) = compiled.create_unwind_info(&*self.compiler.isa) {
                unsafe {
                    UnwindHandle::new(
                        &*self.compiler.isa,
                        alloc.as_ptr().addr().get(),
                        &unwind_info,
                    )
                }
            } else {
                None
            };

        // TODO: remove this and deal with handles
        std::mem::forget(unwind_handle);

        let block = Block::new(alloc, meta);
        self.compiled_count += 1;

        Ok(block)
    }

    /// Calls the given block with the given context.
    ///
    /// # Safety
    /// `ctx` must match the type expected by the hooks of this JIT context.
    pub unsafe fn call(&mut self, ctx: *mut Context, block: BlockFn) -> Info {
        // SAFETY: the exclusive reference to the context guarantees the allocator is not being
        // used, keeping the allocations safe
        unsafe { self.trampoline.call(ctx, block) }
    }
}
