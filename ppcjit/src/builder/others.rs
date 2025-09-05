use super::BlockBuilder;
use cranelift::prelude::InstBuilder;
use hemicore::arch::{InsExt, Reg, powerpc::Ins};

impl BlockBuilder<'_> {
    pub fn stub(&mut self, _: Ins) {
        // stub
    }

    pub fn mfspr(&mut self, ins: Ins) {
        let value = self.get(ins.spr());
        self.set(ins.gpr_d(), value);
    }

    pub fn mtspr(&mut self, ins: Ins) {
        let value = self.get(ins.gpr_s());
        let spr = ins.spr();

        if spr.is_data_bat() {
            self.dbat_changed = true;
        }

        if spr.is_instr_bat() {
            self.ibat_changed = true;
        }

        self.set(spr, value);
    }

    pub fn mtsr(&mut self, ins: Ins) {
        let value = self.get(ins.gpr_s());
        let sr = Reg::SR[ins.field_sr() as usize];
        self.set(sr, value);
    }

    pub fn mfmsr(&mut self, ins: Ins) {
        // TODO: check user mode

        let value = self.get(Reg::MSR);
        self.set(ins.gpr_d(), value);
    }

    pub fn mtmsr(&mut self, ins: Ins) {
        // TODO: check user mode
        // TODO: deal with exception stuff

        let value = self.get(ins.gpr_s());
        self.set(Reg::MSR, value);
    }

    pub fn mtsfb1(&mut self, ins: Ins) {
        let bit = ins.field_crbd();
        let old = self.get(Reg::FPSCR);
        let new = self.bd.ins().bor_imm(old, 1 << bit);
        self.set(Reg::FPSCR, new);

        if ins.field_rc() {
            todo!()
        }
    }
}
