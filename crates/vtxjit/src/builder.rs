mod attr;

use crate::{Compiler, builder::attr::AttributeExt, parser::Config};
use cranelift::{codegen::ir, frontend, prelude::InstBuilder};
use hemisphere::system::gx::{
    Vertex,
    cmd::{
        ArrayDescriptor,
        attributes::{self, AttributeMode},
    },
};
use rustc_hash::FxHashMap;
use seq_macro::seq;
use util::offset_of;

const MEMFLAGS: ir::MemFlags = ir::MemFlags::new().with_notrap().with_can_move();
const MEMFLAGS_READONLY: ir::MemFlags = ir::MemFlags::new()
    .with_notrap()
    .with_can_move()
    .with_readonly();

struct Array {
    base: ir::Value,
    stride: ir::Value,
}

struct Consts {
    ptr_type: ir::Type,

    arrays_ptr: ir::Value,
    ram_ptr: ir::Value,
    data_ptr: ir::Value,
    default_pos: ir::Value,
    default_tex: [ir::Value; 8],
    vertices_ptr: ir::Value,
    mtx_set_ptr: ir::Value,
    count: ir::Value,
}

struct Vars {
    arrays: FxHashMap<usize, Array>,
    data_ptr: ir::Value,
    vertex_ptr: ir::Value,
}

pub struct ParserBuilder<'ctx> {
    bd: frontend::FunctionBuilder<'ctx>,
    config: Config,
    consts: Consts,
    vars: Vars,
    current_bb: ir::Block,
}

impl<'ctx> ParserBuilder<'ctx> {
    pub fn new(
        compiler: &'ctx Compiler,
        mut bd: frontend::FunctionBuilder<'ctx>,
        config: Config,
    ) -> Self {
        let entry_bb = bd.create_block();
        bd.append_block_params_for_function_params(entry_bb);
        bd.switch_to_block(entry_bb);
        bd.seal_block(entry_bb);

        let ptr_type = compiler.isa.pointer_type();
        let params = bd.block_params(entry_bb);
        let ram_ptr = params[0];
        let arrays_ptr = params[1];
        let default_mtx_ptr = params[2];
        let data_ptr = params[3];
        let vertices_ptr = params[4];
        let mtx_map_ptr = params[5];
        let count = params[6];

        // extract default matrix indices
        let default_mtx = bd
            .ins()
            .load(ir::types::I64, MEMFLAGS_READONLY, default_mtx_ptr, 0);
        let mut extract_idx = |i: usize| {
            let shifted = bd.ins().ushr_imm(default_mtx, i as i64 * 6);
            let masked = bd.ins().band_imm(shifted, 0x3F);
            
            bd.ins().ireduce(ir::types::I16, masked)
        };

        let default_pos = extract_idx(0);
        let default_tex: [ir::Value; 8] = std::array::from_fn(|i| extract_idx(i + 1));

        let consts = Consts {
            ptr_type,

            arrays_ptr,
            ram_ptr,
            data_ptr,
            default_pos,
            default_tex,
            vertices_ptr,
            mtx_set_ptr: mtx_map_ptr,
            count,
        };

        let arrays = FxHashMap::default();
        let vars = Vars {
            arrays,
            data_ptr,
            vertex_ptr: vertices_ptr,
        };

        Self {
            bd,
            config,
            consts,
            vars,
            current_bb: entry_bb,
        }
    }

    fn switch_to_bb(&mut self, bb: ir::Block) {
        self.bd.switch_to_block(bb);
        self.current_bb = bb;
    }

    fn shift_mask(&mut self, value: ir::Value, shift: i64, mask: i64) -> ir::Value {
        let shifted = self.bd.ins().ushr_imm(value, shift);
        
        self.bd.ins().band_imm(shifted, mask)
    }

    fn include_matrix(&mut self, is_normal: bool, mat_idx: ir::Value) {
        let mat_idx = self.bd.ins().uextend(self.consts.ptr_type, mat_idx);
        let mat_full_idx = if is_normal {
            self.bd.ins().iadd_imm(mat_idx, 256)
        } else {
            mat_idx
        };

        let word_idx = self.bd.ins().udiv_imm(mat_full_idx, 64);
        let bit_idx = self.bd.ins().urem_imm(mat_full_idx, 64);

        let offset = self.bd.ins().imul_imm(word_idx, 8);
        let ptr = self.bd.ins().iadd(self.consts.mtx_set_ptr, offset);
        let curr = self.bd.ins().load(ir::types::I64, MEMFLAGS, ptr, 0);
        let one = self.bd.ins().iconst(ir::types::I64, 1);
        let bit = self.bd.ins().ishl(one, bit_idx);
        let new = self.bd.ins().bor(curr, bit);
        self.bd.ins().store(MEMFLAGS, new, ptr, 0);
    }

    fn parse_direct<A: AttributeExt>(&mut self) {
        let descriptor = A::get_descriptor(&self.config.vat);
        let consumed = A::parse(&descriptor, self, self.vars.data_ptr);
        self.vars.data_ptr = self.bd.ins().iadd_imm(self.vars.data_ptr, consumed as i64);
    }

    fn parse_indexed<A: AttributeExt>(&mut self, index_ty: ir::Type) {
        let descriptor = A::get_descriptor(&self.config.vat);
        let array = &self.vars.arrays[&A::ARRAY_OFFSET];

        // load index
        let index = self
            .bd
            .ins()
            .load(index_ty, MEMFLAGS_READONLY, self.vars.data_ptr, 0);

        let index = if index_ty.bytes() == 1 {
            index
        } else {
            self.bd.ins().bswap(index)
        };

        let index = self.bd.ins().uextend(ir::types::I32, index);

        // compute address
        let offset = self.bd.ins().imul(index, array.stride);
        let addr = self.bd.ins().iadd(array.base, offset);

        // compute ptr
        let addr = self.bd.ins().uextend(ir::types::I64, addr);
        let ptr = self.bd.ins().iadd(self.consts.ram_ptr, addr);

        // parse
        A::parse(&descriptor, self, ptr);
        self.vars.data_ptr = self
            .bd
            .ins()
            .iadd_imm(self.vars.data_ptr, index_ty.bytes() as i64);
    }

    fn parse<A: AttributeExt>(&mut self) {
        let mode = A::get_mode(&self.config.vcd);
        match mode {
            AttributeMode::None => A::set_default(self),
            AttributeMode::Direct => self.parse_direct::<A>(),
            AttributeMode::Index8 => self.parse_indexed::<A>(ir::types::I8),
            AttributeMode::Index16 => self.parse_indexed::<A>(ir::types::I16),
        }
    }

    fn increment_srcloc(&mut self) {
        let curr = self.bd.srcloc().bits();
        self.bd.set_srcloc(ir::SourceLoc::new(curr + 1));
    }

    fn load_array<A: AttributeExt>(&mut self) {
        let mode = A::get_mode(&self.config.vcd);
        match mode {
            AttributeMode::None => return,
            AttributeMode::Direct => return,
            _ => (),
        }

        // load base
        let base = self.bd.ins().load(
            ir::types::I32,
            MEMFLAGS_READONLY,
            self.consts.arrays_ptr,
            (A::ARRAY_OFFSET + offset_of!(ArrayDescriptor, address)) as i32,
        );

        // load stride
        let stride = self.bd.ins().load(
            ir::types::I32,
            MEMFLAGS_READONLY,
            self.consts.arrays_ptr,
            (A::ARRAY_OFFSET + offset_of!(ArrayDescriptor, stride)) as i32,
        );

        self.vars
            .arrays
            .insert(A::ARRAY_OFFSET, Array { base, stride });
    }

    fn head(&mut self) {
        self.load_array::<attributes::Position>();
        self.load_array::<attributes::Normal>();
        self.load_array::<attributes::Chan0>();
        self.load_array::<attributes::Chan1>();
        seq! {
            N in 0..8 {
                self.load_array::<attributes::TexCoords<N>>();
            }
        }
    }

    fn body(&mut self) {
        self.bd.set_srcloc(ir::SourceLoc::new(0));

        self.parse::<attributes::PosMatrixIndex>();
        seq! {
            N in 0..8 {
                self.increment_srcloc();
                self.parse::<attributes::TexMatrixIndex<N>>();
            }
        }

        self.increment_srcloc();
        self.parse::<attributes::Position>();
        self.increment_srcloc();
        self.parse::<attributes::Normal>();
        self.increment_srcloc();
        self.parse::<attributes::Chan0>();
        self.increment_srcloc();
        self.parse::<attributes::Chan1>();
        seq! {
            N in 0..8 {
                self.increment_srcloc();
                self.parse::<attributes::TexCoords<N>>();
            }
        }

        self.bd.set_srcloc(ir::SourceLoc::default());
    }

    pub fn build(mut self) {
        // setup everything needed before the loop
        self.head();

        // setup the loop
        let iter_bb = self.bd.create_block();
        let body_bb = self.bd.create_block();
        let exit_bb = self.bd.create_block();

        self.bd.set_cold_block(exit_bb);
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
        self.vars.data_ptr = params[0];
        self.vars.vertex_ptr = params[1];
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
        self.switch_to_bb(body_bb);
        self.body();

        // finally, increment everything and start next loop iteration
        self.vars.vertex_ptr = self
            .bd
            .ins()
            .iadd_imm(self.vars.vertex_ptr, size_of::<Vertex>() as i64);
        let loop_iter = self.bd.ins().iadd_imm(loop_iter, 1);
        self.bd.ins().jump(
            iter_bb,
            &[
                ir::BlockArg::Value(self.vars.data_ptr),
                ir::BlockArg::Value(self.vars.vertex_ptr),
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
