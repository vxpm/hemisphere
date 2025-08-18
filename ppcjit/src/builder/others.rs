use super::BlockBuilder;
use crate::builder::{Reg, Spr};
use cranelift::{codegen::ir, prelude::InstBuilder};
use powerpc::Ins;

impl BlockBuilder<'_> {
    pub fn mfspr(&mut self, ins: Ins) {
        let spr = Spr::try_from(ins.field_spr()).unwrap();
        let value = self.get(Reg::Spr(spr));
        self.set(Reg::Gpr(ins.field_rd()), value);
    }

    pub fn mtspr(&mut self, ins: Ins) {
        let spr = Spr::try_from(ins.field_spr()).unwrap();
        let value = self.get(Reg::Gpr(ins.field_rs()));
        self.set(Reg::Spr(spr), value);
    }
}
