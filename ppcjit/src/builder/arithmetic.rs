use super::{BlockBuilder, Reg};
use cranelift::prelude::InstBuilder;
use powerpc::Ins;

impl BlockBuilder<'_> {
    pub fn add(&mut self, ins: Ins) {
        let ra = self.get(Reg::Gpr(ins.field_ra()));
        let rb = self.get(Reg::Gpr(ins.field_rb()));
        let (result, overflowed) = self.bd.ins().sadd_overflow(ra, rb);

        if ins.field_rc() {
            self.update_cr0(result, overflowed);
        }

        self.set(Reg::Gpr(ins.field_rd()), result);
    }
}
