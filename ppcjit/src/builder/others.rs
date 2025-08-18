use super::BlockBuilder;
use crate::builder::{Reg, Spr};
use cranelift::{codegen::ir, prelude::InstBuilder};
use powerpc::Ins;

impl BlockBuilder<'_> {
    pub fn mfspr(&mut self, ins: Ins) {
        let spr = Spr::try_from(ins.field_spr()).unwrap();
        let value = self.bd.ins().load(
            ir::types::I32,
            ir::MemFlags::trusted(),
            self.regs_ptr,
            spr.offset(),
        );

        self.set(Reg::Gpr(ins.field_rd()), value);
    }
}
