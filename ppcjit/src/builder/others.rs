use super::BlockBuilder;
use crate::{
    block::Hooks,
    builder::{Action, Info},
};
use bitos::BitUtils;
use common::arch::{InsExt, Reg, SPR, disasm::Ins};
use cranelift::{codegen::ir, prelude::InstBuilder};
use std::mem::offset_of;

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

fn generate_mask(control: u8) -> u32 {
    let mut mask = 0;
    for i in 0..8 {
        if control.bit(i) {
            mask |= (0xF) << (4 * i);
        }
    }

    mask
}

impl BlockBuilder<'_> {
    pub fn mfspr(&mut self, ins: Ins) -> Info {
        let spr = ins.spr();
        match spr {
            SPR::DEC => self.call_generic_hook(offset_of!(Hooks, dec_read)),
            SPR::TBL | SPR::TBU => self.call_generic_hook(offset_of!(Hooks, tb_read)),
            SPR::WPAR => tracing::warn!("read from WPAR"),
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

        match spr {
            SPR::DEC => self.call_generic_hook(offset_of!(Hooks, dec_changed)),
            SPR::TBL | SPR::TBU => self.call_generic_hook(offset_of!(Hooks, tb_changed)),
            SPR::WPAR => tracing::warn!("write to WPAR"),
            spr if spr.is_data_bat() => self.dbat_changed = true,
            spr if spr.is_instr_bat() => self.ibat_changed = true,
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

        self.call_generic_hook(offset_of!(Hooks, msr_changed));

        MSR_INFO
    }

    pub fn mfcr(&mut self, ins: Ins) -> Info {
        let value = self.get(Reg::CR);
        self.set(ins.gpr_d(), value);

        CR_INFO
    }

    pub fn mtcrf(&mut self, ins: Ins) -> Info {
        let rs = self.get(ins.gpr_s());
        let mask = self.ir_value(generate_mask(ins.field_crm()));

        let cr = self.get(Reg::CR);
        let value = self.bd.ins().bitselect(mask, rs, cr);

        self.set(Reg::CR, value);

        CR_INFO
    }

    pub fn mtfsf(&mut self, ins: Ins) -> Info {
        let fpr_b = self.get(ins.fpr_b());
        let mask = self.ir_value(generate_mask(ins.field_mtfsf_fm()));

        let fpscr = self.get(Reg::FPSCR);
        let bits = self.bd.ins().bitcast(
            ir::types::I64,
            ir::MemFlags::new().with_endianness(ir::Endianness::Little),
            fpr_b,
        );
        let low = self.bd.ins().ireduce(ir::types::I32, bits);

        let value = self.bd.ins().bitselect(mask, low, fpscr);
        self.set(Reg::FPSCR, value);

        self.update_fpscr();

        if ins.field_rc() {
            self.update_cr1_float();
        }

        CR_INFO
    }

    pub fn mftb(&mut self, ins: Ins) -> Info {
        self.call_generic_hook(offset_of!(Hooks, tb_read));

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

    pub fn creqv(&mut self, ins: Ins) -> Info {
        let bit_a = 31 - ins.field_crba();
        let bit_b = 31 - ins.field_crbb();
        let bit_dest = 31 - ins.field_crbd();

        let cr = self.get(Reg::CR);
        let bit_a = self.get_bit(cr, bit_a);
        let bit_b = self.get_bit(cr, bit_b);
        let xored = self.bd.ins().bxor(bit_a, bit_b);
        let compl = self.bd.ins().bnot(xored);

        let value = self.set_bit(cr, bit_dest, compl);
        self.set(Reg::CR, value);

        CR_INFO
    }

    pub fn cror(&mut self, ins: Ins) -> Info {
        let bit_a = 31 - ins.field_crba();
        let bit_b = 31 - ins.field_crbb();
        let bit_dest = 31 - ins.field_crbd();

        let cr = self.get(Reg::CR);
        let bit_a = self.get_bit(cr, bit_a);
        let bit_b = self.get_bit(cr, bit_b);
        let ored = self.bd.ins().bor(bit_a, bit_b);

        let value = self.set_bit(cr, bit_dest, ored);
        self.set(Reg::CR, value);

        CR_INFO
    }

    pub fn crorc(&mut self, ins: Ins) -> Info {
        let bit_a = 31 - ins.field_crba();
        let bit_b = 31 - ins.field_crbb();
        let bit_dest = 31 - ins.field_crbd();

        let cr = self.get(Reg::CR);
        let bit_a = self.get_bit(cr, bit_a);
        let bit_b = self.get_bit(cr, bit_b);
        let not_b = self.bd.ins().bnot(bit_b);
        let ored = self.bd.ins().bor(bit_a, not_b);

        let value = self.set_bit(cr, bit_dest, ored);
        self.set(Reg::CR, value);

        CR_INFO
    }

    pub fn crnor(&mut self, ins: Ins) -> Info {
        let bit_a = 31 - ins.field_crba();
        let bit_b = 31 - ins.field_crbb();
        let bit_dest = 31 - ins.field_crbd();

        let cr = self.get(Reg::CR);
        let bit_a = self.get_bit(cr, bit_a);
        let bit_b = self.get_bit(cr, bit_b);
        let ored = self.bd.ins().bor(bit_a, bit_b);
        let nored = self.bd.ins().bnot(ored);

        let value = self.set_bit(cr, bit_dest, nored);
        self.set(Reg::CR, value);

        CR_INFO
    }

    pub fn crand(&mut self, ins: Ins) -> Info {
        let bit_a = 31 - ins.field_crba();
        let bit_b = 31 - ins.field_crbb();
        let bit_dest = 31 - ins.field_crbd();

        let cr = self.get(Reg::CR);
        let bit_a = self.get_bit(cr, bit_a);
        let bit_b = self.get_bit(cr, bit_b);
        let anded = self.bd.ins().band(bit_a, bit_b);

        let value = self.set_bit(cr, bit_dest, anded);
        self.set(Reg::CR, value);

        CR_INFO
    }

    pub fn crandc(&mut self, ins: Ins) -> Info {
        let bit_a = 31 - ins.field_crba();
        let bit_b = 31 - ins.field_crbb();
        let bit_dest = 31 - ins.field_crbd();

        let cr = self.get(Reg::CR);
        let bit_a = self.get_bit(cr, bit_a);
        let bit_b = self.get_bit(cr, bit_b);
        let not_b = self.bd.ins().bnot(bit_b);
        let anded = self.bd.ins().band(bit_a, not_b);

        let value = self.set_bit(cr, bit_dest, anded);
        self.set(Reg::CR, value);

        CR_INFO
    }

    pub fn crnand(&mut self, ins: Ins) -> Info {
        let bit_a = 31 - ins.field_crba();
        let bit_b = 31 - ins.field_crbb();
        let bit_dest = 31 - ins.field_crbd();

        let cr = self.get(Reg::CR);
        let bit_a = self.get_bit(cr, bit_a);
        let bit_b = self.get_bit(cr, bit_b);
        let anded = self.bd.ins().band(bit_a, bit_b);
        let nanded = self.bd.ins().bnot(anded);

        let value = self.set_bit(cr, bit_dest, nanded);
        self.set(Reg::CR, value);

        CR_INFO
    }

    pub fn mcrf(&mut self, ins: Ins) -> Info {
        let src_field = 7 - ins.field_crfs();
        let dst_field = 7 - ins.field_crfd();

        // get src
        let cr = self.get(Reg::CR);
        let src = self.bd.ins().ushr_imm(cr, 4 * src_field as u64 as i64);
        let src = self.bd.ins().band_imm(src, 0b1111u64 as i64);

        // place src in dst
        let new = self.bd.ins().ishl_imm(src, 4 * dst_field as u64 as i64);
        let dst_mask = self.ir_value(0b1111 << (4 * dst_field));
        let value = self.bd.ins().bitselect(dst_mask, new, cr);

        self.set(Reg::CR, value);

        CR_INFO
    }

    pub fn mtfsb1(&mut self, ins: Ins) -> Info {
        let bit = 31 - ins.field_crbd();
        let fpscr = self.get(Reg::FPSCR);

        let value = self.set_bit(fpscr, bit, 1);
        self.set(Reg::FPSCR, value);

        self.update_fpscr();

        if ins.field_rc() {
            self.update_cr1_float();
        }

        CR_INFO
    }
}
