use crate::builder::util::IntoIrValue;

use super::BlockBuilder;
use bitos::{bitos, integer::u5};
use cranelift::{codegen::ir, prelude::InstBuilder};
use common::arch::{Reg, SPR, disasm::Ins};

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

    pub fn b(&mut self, ins: Ins) {
        // NOTE: the minus 4 is to work around the automatic PC increase in the emit method
        let target = self
            .bd
            .ins()
            .iconst(ir::types::I32, (ins.field_li() - 4) as u64 as i64);
        self.setup_jump(!ins.field_aa(), ins.field_lk(), target);
    }

    fn conditional_branch(&mut self, ins: Ins, relative: bool, target: impl IntoIrValue) {
        let options = BranchOptions::from_bits(u5::new(ins.field_bo()));
        let cond_bit = 31 - ins.field_bi();
        let current_pc = self.get(Reg::PC);

        let mut branch = self.const_val(true);
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
        let target = self.const_val(target);
        self.setup_jump(relative, ins.field_lk(), target);
        self.prologue();

        self.bd.switch_to_block(continue_block);
        self.current_bb = continue_block;
        self.set(Reg::PC, current_pc);
    }

    pub fn bc(&mut self, ins: Ins) {
        self.conditional_branch(ins, !ins.field_aa(), ins.field_bd() as i32);
    }

    pub fn bclr(&mut self, ins: Ins) {
        let lr = self.get(SPR::LR);
        self.conditional_branch(ins, false, lr);
    }

    pub fn bcctr(&mut self, ins: Ins) {
        let ctr = self.get(SPR::CTR);
        self.conditional_branch(ins, false, ctr);
    }
}
