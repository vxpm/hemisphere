use super::BlockBuilder;
use crate::{
    block::BlockOutput,
    builder::registers::{Reg, Spr},
};
use bitos::{bitos, integer::u5};
use cranelift::{codegen::ir, prelude::InstBuilder};
use powerpc::Ins;
use std::mem::offset_of;

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
    pub fn setup_jump(&mut self, relative: bool, link: bool, data: u32) {
        let false_ = self.bd.ins().iconst(ir::types::I8, 0);
        let true_ = self.bd.ins().iconst(ir::types::I8, 1);

        self.bd.ins().store(
            ir::MemFlags::trusted(),
            true_,
            self.ctx.output_ptr,
            offset_of!(BlockOutput, jump.execute) as i32,
        );

        self.bd.ins().store(
            ir::MemFlags::trusted(),
            if relative { true_ } else { false_ },
            self.ctx.output_ptr,
            offset_of!(BlockOutput, jump.relative) as i32,
        );

        self.bd.ins().store(
            ir::MemFlags::trusted(),
            if link { true_ } else { false_ },
            self.ctx.output_ptr,
            offset_of!(BlockOutput, jump.link) as i32,
        );

        let data = self.bd.ins().iconst(ir::types::I32, data as u64 as i64);
        self.bd.ins().store(
            ir::MemFlags::trusted(),
            data,
            self.ctx.output_ptr,
            offset_of!(BlockOutput, jump.data) as i32,
        );
    }

    pub fn branch(&mut self, ins: Ins) {
        self.setup_jump(!ins.field_aa(), ins.field_lk(), ins.field_li() as u32);
    }

    pub fn branch_cond(&mut self, ins: Ins) {
        let options = BranchOptions::from_bits(u5::new(ins.field_bo()));
        let cond_bit = 31 - ins.field_bi();

        let mut branch = self.bd.ins().iconst(ir::types::I8, 1);
        if !options.ignore_cr() {
            let cr = self.get(Reg::Cr);
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
            let ctr = self.get(Reg::Spr(Spr::CTR));
            let ctr = self.bd.ins().iadd_imm(ctr, -1);
            self.set(Reg::Spr(Spr::CTR), ctr);

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

        self.bd
            .ins()
            .brif(branch, exit_block, &[], continue_block, &[]);

        self.bd.seal_block(exit_block);
        self.bd.seal_block(continue_block);

        self.bd.switch_to_block(exit_block);
        self.setup_jump(
            !ins.field_aa(),
            ins.field_lk(),
            ins.field_bd() as i32 as u32,
        );
        self.prologue();

        self.bd.switch_to_block(continue_block);
        self.current_bb = continue_block;
    }
}
