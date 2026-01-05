mod builder;
mod parser;

use cranelift::{
    codegen::{self, ir},
    frontend, native,
    prelude::{Configurable, isa::TargetIsa},
};
use hemisphere::{
    modules::vertex::VertexModule,
    system::gx::{
        MatrixSet, Vertex,
        cmd::{Arrays, VertexAttributeStream, VertexDescriptor, attributes::VertexAttributeTable},
        xform::DefaultMatrices,
    },
};
use jitalloc::{Allocator, Exec};
use parser::VertexParser;
use rustc_hash::FxHashMap;
use std::{collections::hash_map::Entry, sync::Arc};

use crate::{builder::ParserBuilder, parser::Config};

struct Compiler {
    isa: Arc<dyn TargetIsa>,
    allocator: Allocator<Exec>,
}

impl Compiler {
    fn new() -> Self {
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

        let isa = isa_builder
            .finish(codegen::settings::Flags::new(codegen))
            .unwrap();

        Compiler {
            isa,
            allocator: Allocator::new(),
        }
    }

    fn parser_signature(&self) -> ir::Signature {
        let ptr = self.isa.pointer_type();
        ir::Signature {
            // ram, arrays, default matrices, data, vertices, matrix map, count
            params: vec![
                ir::AbiParam::new(ptr),
                ir::AbiParam::new(ptr),
                ir::AbiParam::new(ptr),
                ir::AbiParam::new(ptr),
                ir::AbiParam::new(ptr),
                ir::AbiParam::new(ptr),
                ir::AbiParam::new(ir::types::I32),
            ],
            returns: vec![],
            call_conv: codegen::isa::CallConv::SystemV,
        }
    }

    /// Compiles and returns a parser.
    fn compile(
        &mut self,
        code_ctx: &mut codegen::Context,
        func_ctx: &mut frontend::FunctionBuilderContext,
        config: Config,
    ) -> VertexParser {
        let mut func = ir::Function::new();
        func.signature = self.parser_signature();

        let func_builder = frontend::FunctionBuilder::new(&mut func, func_ctx);
        let builder = ParserBuilder::new(self, func_builder, config);
        builder.build();

        println!("{:?}", config);
        println!("{}", func.display());

        code_ctx.clear();
        code_ctx.want_disasm = true;
        code_ctx.func = func;
        code_ctx
            .compile(&*self.isa, &mut Default::default())
            .unwrap();

        let compiled = code_ctx.take_compiled_code().unwrap();
        println!("{}", compiled.vcode.as_ref().unwrap());

        let alloc = self.allocator.allocate(64, compiled.code_buffer());
        VertexParser::new(alloc)
    }
}

pub struct JitVertexModule {
    compiler: Compiler,
    code_ctx: codegen::Context,
    func_ctx: frontend::FunctionBuilderContext,
    parsers: FxHashMap<Config, VertexParser>,
}

unsafe impl Send for JitVertexModule {}

impl JitVertexModule {
    pub fn new() -> Self {
        Self {
            compiler: Compiler::new(),
            code_ctx: codegen::Context::new(),
            func_ctx: frontend::FunctionBuilderContext::new(),
            parsers: FxHashMap::default(),
        }
    }
}

impl VertexModule for JitVertexModule {
    fn parse(
        &mut self,
        ram: &[u8],
        vcd: &VertexDescriptor,
        vat: &VertexAttributeTable,
        arrays: &Arrays,
        default_matrices: &DefaultMatrices,
        stream: &VertexAttributeStream,
        vertices: &mut [std::mem::MaybeUninit<Vertex>],
        matrix_set: &mut MatrixSet,
    ) {
        let config = Config {
            vcd: *vcd,
            vat: *vat,
        }
        .canonicalize();

        let parser = match self.parsers.entry(config) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let parser = self
                    .compiler
                    .compile(&mut self.code_ctx, &mut self.func_ctx, config);

                v.insert(parser)
            }
        };

        let parser = parser.as_ptr();
        parser(
            ram.as_ptr(),
            arrays,
            default_matrices,
            stream.data().as_ptr(),
            vertices.as_mut_ptr().cast(),
            matrix_set,
            stream.count() as u32,
        );
    }
}
