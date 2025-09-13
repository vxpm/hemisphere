use super::BlockBuilder;
use crate::builder::{Info, util::IntoIrValue};
use bitos::{bitos, integer::u5};
use common::arch::{Reg, SPR, disasm::Ins};
use cranelift::{codegen::ir, prelude::InstBuilder};

const JUMP_INFO: Info = Info {
    cycles: 2,
    auto_pc: false,
};

const BRANCH_INFO: Info = Info {
    cycles: 3,
    auto_pc: true,
};

#[bitos(1)]
#[derive(Debug, Clone, Copy)]
enum CtrCond {
    NotEqZero = 0,
    EqZero = 1,
}

#[bitos(5)]
#[derive(Debug)]
struct BranchOptions {
    #[bits(0)]
    likely: bool,
    #[bits(1)]
    ctr_cond: CtrCond,
    #[bits(2)]
    ignore_ctr: bool,
    #[bits(3)]
    desired_cr: bool,
    #[bits(4)]
    ignore_cr: bool,
}

impl BlockBuilder<'_> {
    fn setup_jump(&mut self, relative: bool, link: bool, data: ir::Value) {
        let current_pc = self.get(Reg::PC);
        if link {
            let target = self.bd.ins().iadd_imm(current_pc, 4);
            self.set(SPR::LR, target);
        }

        if relative {
            let target = self.bd.ins().iadd(current_pc, data);
            self.set(Reg::PC, target);
        } else {
            self.set(Reg::PC, data);
        }
    }

    pub fn b(&mut self, ins: Ins) -> Info {
        let target = self.ir_value(ins.field_li());
        self.setup_jump(!ins.field_aa(), ins.field_lk(), target);

        JUMP_INFO
    }

    fn conditional_branch(&mut self, ins: Ins, relative: bool, target: impl IntoIrValue) -> Info {
        let options = BranchOptions::from_bits(u5::new(ins.field_bo()));
        let cond_bit = 31 - ins.field_bi();
        let current_pc = self.get(Reg::PC);

        let mut branch = self.ir_value(true);
        if !options.ignore_cr() {
            let cr = self.get(Reg::CR);
            let cond = self.bd.ins().band_imm(cr, 1 << cond_bit);

            let cond_ok = match options.desired_cr() {
                true => self
                    .bd
                    .ins()
                    .icmp_imm(ir::condcodes::IntCC::UnsignedGreaterThan, cond, 0),
                false => self.bd.ins().icmp_imm(ir::condcodes::IntCC::Equal, cond, 0),
            };

            branch = self.bd.ins().band(branch, cond_ok);
        }

        if !options.ignore_ctr() {
            let ctr = self.get(SPR::CTR);
            let ctr = self.bd.ins().iadd_imm(ctr, -1);
            self.set(SPR::CTR, ctr);

            let ctr_ok = match options.ctr_cond() {
                CtrCond::NotEqZero => {
                    let eq = self.bd.ins().icmp_imm(ir::condcodes::IntCC::Equal, ctr, 0);
                    self.bd.ins().bnot(eq)
                }
                CtrCond::EqZero => self.bd.ins().icmp_imm(ir::condcodes::IntCC::Equal, ctr, 0),
            };

            branch = self.bd.ins().band(branch, ctr_ok);
        }

        let exit_block = self.bd.create_block();
        let continue_block = self.bd.create_block();

        if !(options.ignore_ctr() && options.ignore_cr()) {
            self.bd.set_cold_block(if options.likely() {
                continue_block
            } else {
                exit_block
            });
        }

        self.bd
            .ins()
            .brif(branch, exit_block, &[], continue_block, &[]);

        self.bd.seal_block(exit_block);
        self.bd.seal_block(continue_block);

        self.bd.switch_to_block(exit_block);
        let target = self.ir_value(target);
        self.setup_jump(relative, ins.field_lk(), target);

        // HACK: add to cycles for prologue emission
        self.cycles += BRANCH_INFO.cycles as u32;
        self.prologue();
        self.cycles -= BRANCH_INFO.cycles as u32;

        self.bd.switch_to_block(continue_block);
        self.current_bb = continue_block;
        self.set(Reg::PC, current_pc);

        BRANCH_INFO
    }

    pub fn bc(&mut self, ins: Ins) -> Info {
        self.conditional_branch(ins, !ins.field_aa(), ins.field_bd() as i32)
    }

    pub fn bclr(&mut self, ins: Ins) -> Info {
        let lr = self.get(SPR::LR);
        self.conditional_branch(ins, false, lr)
    }

    pub fn bcctr(&mut self, ins: Ins) -> Info {
        let ctr = self.get(SPR::CTR);
        self.conditional_branch(ins, false, ctr)
    }
}
