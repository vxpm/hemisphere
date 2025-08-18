use super::{BlockBuilder, Reg};
use cranelift::{codegen::ir, prelude::InstBuilder};
use powerpc::Ins;

impl BlockBuilder<'_> {
    pub fn addis(&mut self, ins: Ins) {
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, (ins.field_simm() as i64) << 16);

        let value = if ins.field_ra() == 0 {
            imm
        } else {
            let ra = self.get(Reg::Gpr(ins.field_ra()));
            self.bd.ins().iadd(ra, imm)
        };

        self.set(Reg::Gpr(ins.field_rd()), value);
    }

    pub fn add(&mut self, ins: Ins) {
        let ra = self.get(Reg::Gpr(ins.field_ra()));
        let rb = self.get(Reg::Gpr(ins.field_rb()));
        let (result, overflowed) = self.bd.ins().sadd_overflow(ra, rb);

        if ins.field_rc() {
            self.update_cr0(result, overflowed);
        }

        self.set(Reg::Gpr(ins.field_rd()), result);
    }

    pub fn ori(&mut self, ins: Ins) {
        let imm = self
            .bd
            .ins()
            .iconst(ir::types::I32, ins.field_uimm() as u64 as i64);
        let rs = self.get(Reg::Gpr(ins.field_rs()));

        let value = self.bd.ins().bor(rs, imm);
        self.set(Reg::Gpr(ins.field_ra()), value);
    }
}
