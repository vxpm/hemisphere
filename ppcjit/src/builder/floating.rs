use super::BlockBuilder;
use crate::builder::{Action, Info};
use common::arch::{InsExt, disasm::Ins};

const FLOAT_INFO: Info = Info {
    cycles: 1,
    auto_pc: true,
    action: Action::Continue,
};

impl BlockBuilder<'_> {
    pub fn fmr(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let fb = self.get(ins.fpr_b());
        self.set(ins.fpr_d(), fb);

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
