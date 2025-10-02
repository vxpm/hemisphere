use super::BlockBuilder;
use crate::builder::{Action, Info};
use common::arch::{InsExt, disasm::Ins};
use cranelift::{codegen::ir, prelude::InstBuilder};

const FLOAT_INFO: Info = Info {
    cycles: 1,
    auto_pc: true,
    action: Action::Continue,
};

impl BlockBuilder<'_> {
    pub fn fmr(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let fpr_b = self.get(ins.fpr_b());
        self.set(ins.fpr_d(), fpr_b);

        if ins.field_rc() {
            self.update_cr1_float();
        }

        FLOAT_INFO
    }

    pub fn frsp(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let fpr_b = self.get(ins.fpr_b());

        let single = self.bd.ins().fdemote(ir::types::F32, fpr_b);
        let double = self.bd.ins().fpromote(ir::types::F64, single);
        self.set(ins.fpr_d(), double);

        self.update_fprf_cmpz(double);

        if ins.field_rc() {
            self.update_cr1_float();
        }

        FLOAT_INFO
    }

    pub fn ps_mr(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let ps_b = self.get_ps(ins.fpr_b());
        self.set_ps(ins.fpr_d(), ps_b);

        FLOAT_INFO
    }
}
