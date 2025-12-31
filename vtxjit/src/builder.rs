use crate::{Compiler, parser::Config};
use cranelift::{codegen::ir, frontend, prelude::InstBuilder};
use hemisphere::system::gx::Vertex;

struct Consts {
    ptr_type: ir::Type,

    ram_ptr: ir::Value,
    arrays_ptr: ir::Value,
    default_mtx_ptr: ir::Value,
    data_ptr: ir::Value,
    vertices_ptr: ir::Value,
    mtx_map_ptr: ir::Value,
    count: ir::Value,
}

pub struct ParserBuilder<'ctx> {
    compiler: &'ctx mut Compiler,
    bd: frontend::FunctionBuilder<'ctx>,
    config: Config,
    consts: Consts,
    current_bb: ir::Block,
}

impl<'ctx> ParserBuilder<'ctx> {
    pub fn new(
        compiler: &'ctx mut Compiler,
        mut builder: frontend::FunctionBuilder<'ctx>,
        config: Config,
    ) -> Self {
        let entry_bb = builder.create_block();
        builder.append_block_params_for_function_params(entry_bb);
        builder.switch_to_block(entry_bb);
        builder.seal_block(entry_bb);

        let ptr_type = compiler.isa.pointer_type();
        let params = builder.block_params(entry_bb);
        let ram_ptr = params[0];
        let arrays_ptr = params[1];
        let default_mtx_ptr = params[2];
        let data_ptr = params[3];
        let vertices_ptr = params[4];
        let mtx_map_ptr = params[5];
        let count = params[6];

        let consts = Consts {
            ptr_type,

            ram_ptr,
            arrays_ptr,
            default_mtx_ptr,
            data_ptr,
            vertices_ptr,
            mtx_map_ptr,
            count,
        };

        Self {
            compiler,
            bd: builder,
            config,
            consts,
            current_bb: entry_bb,
        }
    }

    fn switch_to_bb(&mut self, bb: ir::Block) {
        self.bd.switch_to_block(bb);
        self.current_bb = bb;
    }

    pub fn build(mut self) {
        // setup the loop
        let iter_bb = self.bd.create_block();
        let body_bb = self.bd.create_block();
        let exit_bb = self.bd.create_block();

        self.bd.append_block_param(iter_bb, self.consts.ptr_type); // data ptr
        self.bd.append_block_param(iter_bb, self.consts.ptr_type); // vertex ptr
        self.bd.append_block_param(iter_bb, ir::types::I32); // loop iter

        let zero = self.bd.ins().iconst(ir::types::I32, 0);
        self.bd.ins().jump(
            iter_bb,
            &[
                ir::BlockArg::Value(self.consts.data_ptr),
                ir::BlockArg::Value(self.consts.vertices_ptr),
                ir::BlockArg::Value(zero),
            ],
        );

        // loop body: parse a single vertex
        self.switch_to_bb(iter_bb);
        let params = self.bd.block_params(iter_bb);
        let data_ptr = params[0];
        let vertex_ptr = params[1];
        let loop_iter = params[2];

        // first, check if loop iter < count, otherwise exit
        let loop_cond = self.bd.ins().icmp(
            ir::condcodes::IntCC::UnsignedLessThan,
            loop_iter,
            self.consts.count,
        );
        self.bd.ins().brif(loop_cond, body_bb, &[], exit_bb, &[]);

        self.bd.seal_block(body_bb);
        self.bd.seal_block(exit_bb);

        // then, actually parse it
        // TODO - somehow parse
        self.switch_to_bb(body_bb);

        // finally, increment everything and start next loop iteration
        let vertex_ptr = self
            .bd
            .ins()
            .iadd_imm(vertex_ptr, size_of::<Vertex>() as i64);
        let loop_iter = self.bd.ins().iadd_imm(loop_iter, 1);
        self.bd.ins().jump(
            iter_bb,
            &[
                ir::BlockArg::Value(data_ptr),
                ir::BlockArg::Value(vertex_ptr),
                ir::BlockArg::Value(loop_iter),
            ],
        );

        self.bd.seal_block(iter_bb);

        // exit
        self.switch_to_bb(exit_bb);
        self.bd.ins().return_(&[]);
        self.bd.finalize();
    }
}
