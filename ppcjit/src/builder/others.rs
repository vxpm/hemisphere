use super::BlockBuilder;
use bitos::BitUtils;
use common::arch::{InsExt, Reg, disasm::Ins};
use cranelift::{codegen::ir, prelude::InstBuilder};
use tracing::debug;

impl BlockBuilder<'_> {
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

    pub fn mfcr(&mut self, ins: Ins) {
        let value = self.get(Reg::CR);
        self.set(ins.gpr_d(), value);
    }

    pub fn mtcrf(&mut self, ins: Ins) {
        let rs = self.get(ins.gpr_s());
        let mask_control = ins.field_crm();

        let mut mask = 0;
        for i in 0..8 {
            // CR0 is the higher one
            if mask_control.bit(7 - i) {
                mask |= (0xF) << (4 * i);
            }
        }

        debug!("mask control: {:08b} mask: {:032b}", mask_control, mask);

        let cr = self.get(Reg::CR);
        let mask = self.ir_value(mask);
        let value = self.bd.ins().bitselect(mask, rs, cr);

        self.set(Reg::CR, value);
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

    pub fn mftb(&mut self, ins: Ins) {
        let tb = match ins.field_tbr() {
            268 => Reg::TBL,
            269 => Reg::TBU,
            _ => todo!(),
        };

        let value = self.get(tb);

        // increment time base
        let tbl = self.get(Reg::TBL);
        let tbu = self.get(Reg::TBU);

        let one = self.ir_value(1);
        let (new_tbl, ov) = self.bd.ins().uadd_overflow(tbl, one);
        let ov = self.bd.ins().uextend(ir::types::I32, ov);
        let new_tbu = self.bd.ins().iadd(tbu, ov);

        self.set(Reg::TBL, new_tbl);
        self.set(Reg::TBU, new_tbu);
        self.set(ins.gpr_d(), value);
    }
}
