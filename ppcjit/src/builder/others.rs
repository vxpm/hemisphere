use std::mem::offset_of;

use super::BlockBuilder;
use crate::{
    block::Hooks,
    builder::{Action, Info},
};
use bitos::BitUtils;
use common::arch::{InsExt, Reg, SPR, disasm::Ins};
use cranelift::prelude::InstBuilder;
use tracing::debug;

const SPR_INFO: Info = Info {
    cycles: 1,
    auto_pc: true,
    action: Action::Continue,
};

const MSR_INFO: Info = Info {
    cycles: 1,
    auto_pc: true,
    action: Action::Continue,
};

const CR_INFO: Info = Info {
    cycles: 1,
    auto_pc: true,
    action: Action::Continue,
};

const SR_INFO: Info = Info {
    cycles: 2,
    auto_pc: true,
    action: Action::Continue,
};

const TB_INFO: Info = Info {
    cycles: 1,
    auto_pc: true,
    action: Action::Continue,
};

impl BlockBuilder<'_> {
    pub fn mfspr(&mut self, ins: Ins) -> Info {
        let spr = ins.spr();
        match spr {
            SPR::DEC => self.call_generic_hook(offset_of!(Hooks, dec_read) as i32),
            SPR::TBL | SPR::TBU => self.call_generic_hook(offset_of!(Hooks, tb_read) as i32),
            _ => (),
        }

        let value = self.get(spr);
        self.set(ins.gpr_d(), value);

        SPR_INFO
    }

    pub fn mtspr(&mut self, ins: Ins) -> Info {
        let value = self.get(ins.gpr_s());
        let spr = ins.spr();
        self.set(spr, value);

        if spr.is_data_bat() {
            self.dbat_changed = true;
        }

        if spr.is_instr_bat() {
            self.ibat_changed = true;
        }

        match spr {
            SPR::DEC => self.call_generic_hook(offset_of!(Hooks, dec_changed) as i32),
            SPR::TBL | SPR::TBU => self.call_generic_hook(offset_of!(Hooks, tb_changed) as i32),
            _ => (),
        }

        SPR_INFO
    }

    pub fn mtsr(&mut self, ins: Ins) -> Info {
        let value = self.get(ins.gpr_s());
        let sr = Reg::SR[ins.field_sr() as usize];
        self.set(sr, value);

        SR_INFO
    }

    pub fn mfmsr(&mut self, ins: Ins) -> Info {
        let value = self.get(Reg::MSR);
        self.set(ins.gpr_d(), value);

        MSR_INFO
    }

    pub fn mtmsr(&mut self, ins: Ins) -> Info {
        // TODO: deal with exception stuff

        let value = self.get(ins.gpr_s());
        self.set(Reg::MSR, value);

        MSR_INFO
    }

    pub fn mfcr(&mut self, ins: Ins) -> Info {
        let value = self.get(Reg::CR);
        self.set(ins.gpr_d(), value);

        CR_INFO
    }

    pub fn mtcrf(&mut self, ins: Ins) -> Info {
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

        CR_INFO
    }

    pub fn mftb(&mut self, ins: Ins) -> Info {
        self.call_generic_hook(offset_of!(Hooks, tb_read) as i32);

        let tb = match ins.field_tbr() {
            268 => Reg::TBL,
            269 => Reg::TBU,
            _ => todo!(),
        };

        let value = self.get(tb);
        self.set(ins.gpr_d(), value);

        TB_INFO
    }

    pub fn crxor(&mut self, ins: Ins) -> Info {
        let bit_a = 31 - ins.field_crba();
        let bit_b = 31 - ins.field_crbb();
        let bit_dest = 31 - ins.field_crbd();

        let cr = self.get(Reg::CR);
        let bit_a = self.get_bit(cr, bit_a);
        let bit_b = self.get_bit(cr, bit_b);
        let xored = self.bd.ins().bxor(bit_a, bit_b);

        let value = self.set_bit(cr, bit_dest, xored);
        self.set(Reg::CR, value);

        CR_INFO
    }
}
