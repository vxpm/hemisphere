mod builder;
mod sequence;

use crate::builder::BlockBuilder;
use cranelift::{
    codegen::{self, ir},
    frontend, jit,
    module::{self, Module},
    native,
    prelude::Configurable,
};

pub use sequence::Sequence;

#[repr(C)]
pub struct Registers {
    pub gpr: [u32; 32],
    pub fpr: [f32; 32],
}

struct Context {
    codegen: codegen::Context,
    func: frontend::FunctionBuilderContext,
}

pub struct JIT {
    module: jit::JITModule,
    data_description: module::DataDescription,
    ctx: Context,
}

impl Default for JIT {
    fn default() -> Self {
        let mut flag_builder = codegen::settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();

        let isa_builder = native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder
            .finish(codegen::settings::Flags::new(flag_builder))
            .unwrap();

        let builder = jit::JITBuilder::with_isa(isa, module::default_libcall_names());
        let module = jit::JITModule::new(builder);
        let codegen = module.make_context();

        Self {
            module,
            data_description: module::DataDescription::new(),
            ctx: Context {
                codegen,
                func: frontend::FunctionBuilderContext::new(),
            },
        }
    }
}

impl JIT {
    fn block_signature(&self) -> ir::Signature {
        let ptr = self.module.isa().pointer_type();
        ir::Signature {
            params: vec![ir::AbiParam::new(ptr)],
            returns: vec![],
            call_conv: codegen::isa::CallConv::SystemV,
        }
    }

    pub fn build(&mut self, sequence: Sequence) {
        let signature = self.block_signature();
        self.ctx.codegen.func.signature = signature.clone();

        let mut builder = BlockBuilder::new(&mut self.ctx);
        for ins in sequence.iter().copied() {
            builder.emit(ins);
        }
        builder.finish();

        let func = self.module.declare_anonymous_function(&signature).unwrap();

        todo!()
    }
}
