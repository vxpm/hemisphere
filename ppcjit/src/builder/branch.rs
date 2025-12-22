use std::alloc::Layout;

use super::BlockBuilder;
use crate::{
    block::LinkData,
    builder::{Action, InstructionInfo, util::IntoIrValue},
};
use bitos::{bitos, integer::u5};
use cranelift::{codegen::ir, prelude::InstBuilder};
use gekko::{Reg, SPR, disasm::Ins};

const UNCONDITIONAL_BRANCH_INFO: InstructionInfo = InstructionInfo {
    cycles: 2,
    auto_pc: false,
    action: Action::Finish,
};

const CONDITIONAL_BRANCH_INFO: InstructionInfo = InstructionInfo {
    cycles: 2,
    auto_pc: true,
    action: Action::Continue,
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

impl BranchOptions {
    fn is_unconditional(&self) -> bool {
        self.ignore_ctr() && self.ignore_cr()
    }
}

impl BlockBuilder<'_> {
    fn jump_with_block_link(&mut self, destination: ir::Value) {
        // allocate some storage for the link data
        let link_data = self
            .compiler
            .module
            .allocate_data(Layout::new::<Option<LinkData>>());

        // SAFETY: pointer is valid
        unsafe { link_data.as_ptr().cast::<Option<LinkData>>().write(None) };

        self.update_info();
        self.flush();

        // call follow link hook
        // SAFETY: allocator safety enforced by the contract of blocks
        let link_data_ptr = self.bd.ins().iconst(self.consts.ptr_type, unsafe {
            link_data.as_ptr().addr().get() as i64
        });

        let follow_link_hook = self.bd.ins().iconst(
            self.consts.ptr_type,
            self.compiler.hooks.follow_link as usize as i64,
        );

        let inst = self.bd.ins().call_indirect(
            self.consts.signatures.follow_link_hook,
            follow_link_hook,
            &[self.consts.info_ptr, self.consts.ctx_ptr, link_data_ptr],
        );

        let should_follow_link = self.bd.inst_results(inst)[0];
        let follow_link = self.bd.create_block();
        let exit = self.bd.create_block();

        self.bd
            .ins()
            .brif(should_follow_link, follow_link, &[], exit, &[]);

        self.bd.seal_block(follow_link);
        self.bd.seal_block(exit);
        self.bd.set_cold_block(exit);

        // => dont follow link, exit
        self.bd.switch_to_block(exit);
        self.set(Reg::PC, destination);
        self.flush();
        self.prologue();

        // => follow link
        self.bd.switch_to_block(follow_link);
        self.set(Reg::PC, destination);
        self.flush();

        // do we need to link?
        let stored_link = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            link_data_ptr,
            0,
        );

        let call_linked = self.bd.create_block();
        let need_to_link = self.bd.create_block();
        let link_failure = self.bd.create_block();
        self.bd.set_cold_block(need_to_link);
        self.bd.set_cold_block(link_failure);

        self.bd
            .append_block_param(call_linked, self.consts.ptr_type);

        self.bd.ins().brif(
            stored_link,
            call_linked,
            &[ir::BlockArg::Value(stored_link)],
            need_to_link,
            &[],
        );

        self.bd.seal_block(need_to_link);

        // => need to link
        self.bd.switch_to_block(need_to_link);

        // call try link hook
        let try_link_hook = self.bd.ins().iconst(
            self.consts.ptr_type,
            self.compiler.hooks.try_link as usize as i64,
        );

        self.bd.ins().call_indirect(
            self.consts.signatures.try_link_hook,
            try_link_hook,
            &[self.consts.ctx_ptr, destination, link_data_ptr],
        );

        // was the link successful?
        let stored_link = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            link_data_ptr,
            0,
        );

        self.bd.ins().brif(
            stored_link,
            call_linked,
            &[ir::BlockArg::Value(stored_link)],
            link_failure,
            &[],
        );

        self.bd.seal_block(call_linked);
        self.bd.seal_block(link_failure);

        // => call linked
        self.bd.switch_to_block(call_linked);
        let link = self.bd.block_params(call_linked)[0];
        self.bd.ins().return_call_indirect(
            self.consts.signatures.block,
            link,
            &[
                self.consts.info_ptr,
                self.consts.ctx_ptr,
                self.consts.regs_ptr,
            ],
        );

        // => link failure
        self.bd.switch_to_block(link_failure);
        self.prologue();
    }

    fn jump(&mut self, relative: bool, link_register: bool, block_link: bool, data: ir::Value) {
        let current_pc = self.get(Reg::PC);
        let destination = if relative {
            self.bd.ins().iadd(current_pc, data)
        } else {
            data
        };

        if link_register {
            let ret_addr = self.bd.ins().iadd_imm(current_pc, 4);
            self.set(SPR::LR, ret_addr);
        }

        self.executed_instructions += 1;
        self.executed_cycles += 2;

        if block_link {
            self.jump_with_block_link(destination);
        } else {
            self.set(Reg::PC, destination);
            self.flush();
            self.prologue();
        }

        self.executed_instructions -= 1;
        self.executed_cycles -= 2;
    }

    pub fn b(&mut self, ins: Ins) -> InstructionInfo {
        let destination = self.ir_value(ins.field_li());
        self.jump(!ins.field_aa(), ins.field_lk(), true, destination);
        UNCONDITIONAL_BRANCH_INFO
    }

    fn branch(
        &mut self,
        ins: Ins,
        relative: bool,
        block_link: bool,
        target: impl IntoIrValue,
    ) -> InstructionInfo {
        let options = BranchOptions::from_bits(u5::new(ins.field_bo()));
        let target = self.ir_value(target);

        if options.is_unconditional() {
            self.jump(relative, ins.field_lk(), block_link, target);
            UNCONDITIONAL_BRANCH_INFO
        } else {
            let cond_bit = 31 - ins.field_bi();
            let current_pc = self.get(Reg::PC);

            let mut branch = self.ir_value(true);
            if !options.ignore_cr() {
                let cr = self.get(Reg::CR);

                let bit = self.get_bit(cr, cond_bit);
                let cond_ok = if options.desired_cr() {
                    bit
                } else {
                    self.bd.ins().bxor_imm(bit, 1)
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

            // => exit (take branch)
            self.switch_to_bb(exit_block);
            let target = self.ir_value(target);
            self.jump(relative, ins.field_lk(), block_link, target);

            // => continue (do not take branch)
            self.switch_to_bb(continue_block);
            self.current_bb = continue_block;

            // undo PC change from `setup_jump`
            self.set(Reg::PC, current_pc);

            CONDITIONAL_BRANCH_INFO
        }
    }

    pub fn bc(&mut self, ins: Ins) -> InstructionInfo {
        self.branch(ins, !ins.field_aa(), true, ins.field_bd() as i32)
    }

    pub fn bclr(&mut self, ins: Ins) -> InstructionInfo {
        let lr = self.get(SPR::LR);
        self.branch(ins, false, false, lr)
    }

    pub fn bcctr(&mut self, ins: Ins) -> InstructionInfo {
        let ctr = self.get(SPR::CTR);
        self.branch(ins, false, false, ctr)
    }
}
