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
            todo!("copy FPSCR to CR1");
        }

        FLOAT_INFO
    }
}
