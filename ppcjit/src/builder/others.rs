use super::BlockBuilder;
use hemicore::arch::{InsExt, powerpc::Ins};

impl BlockBuilder<'_> {
    pub fn mfspr(&mut self, ins: Ins) {
        let value = self.get(ins.spr());
        self.set(ins.gpr_d(), value);
    }

    pub fn mtspr(&mut self, ins: Ins) {
        let value = self.get(ins.gpr_s());
        self.set(ins.spr(), value);
    }
}
