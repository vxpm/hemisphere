use super::BlockBuilder;
use crate::block::BlockOutput;
use cranelift::{codegen::ir, prelude::InstBuilder};
use powerpc::Ins;
use std::mem::offset_of;

impl BlockBuilder<'_> {
    pub fn setup_jump(&mut self, relative: bool, link: bool, data: u32) {
        let false_ = self.bd.ins().iconst(ir::types::I8, 0);
        let true_ = self.bd.ins().iconst(ir::types::I8, 1);

        self.bd.ins().store(
            ir::MemFlags::trusted(),
            true_,
            self.ctx.output_ptr,
            offset_of!(BlockOutput, jump.execute) as i32,
        );

        self.bd.ins().store(
            ir::MemFlags::trusted(),
            if relative { true_ } else { false_ },
            self.ctx.output_ptr,
            offset_of!(BlockOutput, jump.relative) as i32,
        );

        self.bd.ins().store(
            ir::MemFlags::trusted(),
            if link { true_ } else { false_ },
            self.ctx.output_ptr,
            offset_of!(BlockOutput, jump.link) as i32,
        );

        let data = self.bd.ins().iconst(ir::types::I32, data as u64 as i64);
        self.bd.ins().store(
            ir::MemFlags::trusted(),
            data,
            self.ctx.output_ptr,
            offset_of!(BlockOutput, jump.data) as i32,
        );
    }

    pub fn branch(&mut self, ins: Ins) {
        self.setup_jump(!ins.field_aa(), ins.field_lk(), ins.field_li() as u32);
    }
}
