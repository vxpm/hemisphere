use super::BlockBuilder;
use hemicore::arch::{InsExt, Reg, powerpc::Ins};

impl BlockBuilder<'_> {
    pub fn mfspr(&mut self, ins: Ins) {
        let value = self.get(ins.spr());
        self.set(ins.gpr_d(), value);
    }

    pub fn mtspr(&mut self, ins: Ins) {
        let value = self.get(ins.gpr_s());
        self.set(ins.spr(), value);
    }

    pub fn mfmsr(&mut self, ins: Ins) {
        // TODO: check user mode

        let value = self.get(Reg::MSR);
        self.set(ins.gpr_d(), value);
    }
}
