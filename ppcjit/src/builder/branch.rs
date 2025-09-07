use super::BlockBuilder;
use bitos::{bitos, integer::u5};
use cranelift::{codegen::ir, prelude::InstBuilder};
use hemicore::arch::{Reg, SPR, powerpc::Ins};
use tracing::debug;

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

    fn setup_jump_imm(&mut self, relative: bool, link: bool, data: i32) {
        let data = self.bd.ins().iconst(ir::types::I32, data as i64);
        self.setup_jump(relative, link, data);
    }

    pub fn branch(&mut self, ins: Ins) {
        // NOTE: the minus 4 is to work around the automatic PC increase in the emit method
        self.setup_jump_imm(!ins.field_aa(), ins.field_lk(), ins.field_li() - 4);
    }

    pub fn branch_cond(&mut self, ins: Ins) {
        let options = BranchOptions::from_bits(u5::new(ins.field_bo()));
        let cond_bit = 31 - ins.field_bi();
        let current_pc = self.get(Reg::PC);

        let mut branch = self.bd.ins().iconst(ir::types::I8, 1);
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
                    self.bd
                        .ins()
                        .icmp_imm(ir::condcodes::IntCC::UnsignedGreaterThan, ctr, 0)
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
        self.setup_jump_imm(!ins.field_aa(), ins.field_lk(), ins.field_bd() as i32);
        self.prologue();

        self.bd.switch_to_block(continue_block);
        self.current_bb = continue_block;
        self.set(Reg::PC, current_pc);
    }

    pub fn branch_cond_lr(&mut self, ins: Ins) {
        let options = BranchOptions::from_bits(u5::new(ins.field_bo()));
        let cond_bit = 31 - ins.field_bi();
        let addr = self.get(SPR::LR);
        let current_pc = self.get(Reg::PC);

        let mut branch = self.bd.ins().iconst(ir::types::I8, 1);
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
                    self.bd
                        .ins()
                        .icmp_imm(ir::condcodes::IntCC::UnsignedGreaterThan, ctr, 0)
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
        self.setup_jump(false, ins.field_lk(), addr);
        self.prologue();

        self.bd.switch_to_block(continue_block);
        self.current_bb = continue_block;
        self.set(Reg::PC, current_pc);
    }

    pub fn branch_cond_ctr(&mut self, ins: Ins) {
        let options = BranchOptions::from_bits(u5::new(ins.field_bo()));
        let cond_bit = 31 - ins.field_bi();
        let current_pc = self.get(Reg::PC);

        debug!("bcctr: {options:?}");

        let mut branch = self.bd.ins().iconst(ir::types::I8, 1);
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
                    self.bd
                        .ins()
                        .icmp_imm(ir::condcodes::IntCC::UnsignedGreaterThan, ctr, 0)
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
        let ctr = self.get(SPR::CTR);
        let target = self.bd.ins().band_imm(ctr, !0b11);
        self.setup_jump(false, ins.field_lk(), target);
        self.prologue();

        self.bd.switch_to_block(continue_block);
        self.current_bb = continue_block;
        self.set(Reg::PC, current_pc);
    }
}
