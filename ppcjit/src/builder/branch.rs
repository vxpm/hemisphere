use std::mem::offset_of;

use super::BlockBuilder;
use crate::{
    block::{Hooks, Info},
    builder::{Action, InstructionInfo, util::IntoIrValue},
};
use bitos::{bitos, integer::u5};
use cranelift::{codegen::ir, module::Module, prelude::InstBuilder};
use gekko::{Reg, SPR, disasm::Ins};

const UNCONDITIONAL_BRANCH_INFO: InstructionInfo = InstructionInfo {
    cycles: 2,
    auto_pc: false,
    action: Action::Finish,
};

const CONDITIONAL_BRANCH_INFO: InstructionInfo = InstructionInfo {
    cycles: 3,
    auto_pc: false,
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
    /// Updates the Info struct.
    fn update_info(&mut self) {
        let instructions = self.ir_value(self.executed);
        let cycles = self.ir_value(self.cycles);

        self.bd.ins().store(
            ir::MemFlags::trusted(),
            instructions,
            self.consts.info_ptr,
            offset_of!(Info, instructions) as i32,
        );

        self.bd.ins().store(
            ir::MemFlags::trusted(),
            cycles,
            self.consts.info_ptr,
            offset_of!(InstructionInfo, cycles) as i32,
        );
    }

    fn jump_with_block_link(&mut self, destination: ir::Value) {
        // define storage for the link
        let link_storage = self.module.declare_anonymous_data(true, false).unwrap();
        self.module
            .define_data(
                link_storage,
                &cranelift::module::DataDescription {
                    init: cranelift::module::Init::Zeros {
                        size: size_of::<usize>(),
                    },
                    ..cranelift::module::DataDescription::new()
                },
            )
            .unwrap();
        let link_storage = self
            .module
            .declare_data_in_func(link_storage, &mut self.bd.func);

        self.update_info();
        self.flush();

        // call follow link hook
        let follow_link_hook = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            self.consts.hooks_ptr,
            offset_of!(Hooks, follow_link) as i32,
        );

        let follow_link_sig = *self
            .consts
            .hooks_sig
            .entry(offset_of!(Hooks, follow_link) as i32)
            .or_insert_with(|| {
                self.bd
                    .import_signature(Hooks::follow_link_sig(self.consts.ptr_type))
            });

        let inst = self.bd.ins().call_indirect(
            follow_link_sig,
            follow_link_hook,
            &[self.consts.info_ptr, self.consts.ctx_ptr],
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

        // => exit
        self.bd.switch_to_block(exit);
        self.set(Reg::PC, destination);
        self.flush();
        self.prologue();

        // => follow link
        self.bd.switch_to_block(follow_link);

        self.set(Reg::PC, destination);
        self.flush();

        // do we need to link?
        let link_storage_ptr = self
            .bd
            .ins()
            .global_value(self.consts.ptr_type, link_storage);

        let stored_link = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            link_storage_ptr,
            0,
        );

        let need_to_link = self.bd.create_block();
        let is_linked = self.bd.create_block();

        self.bd
            .ins()
            .brif(stored_link, is_linked, &[], need_to_link, &[]);

        self.bd.seal_block(is_linked);
        self.bd.seal_block(need_to_link);
        self.bd.set_cold_block(need_to_link);

        // => follow link => is linked
        self.bd.switch_to_block(is_linked);
        self.bd.ins().return_call_indirect(
            self.consts.block_sig,
            stored_link,
            &[
                self.consts.info_ptr,
                self.consts.ctx_ptr,
                self.consts.hooks_ptr,
            ],
        );

        // => follow link => need to link
        self.bd.switch_to_block(need_to_link);

        // call try link hook
        let try_link_hook = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            self.consts.hooks_ptr,
            offset_of!(Hooks, try_link) as i32,
        );

        let try_link_sig = *self
            .consts
            .hooks_sig
            .entry(offset_of!(Hooks, try_link) as i32)
            .or_insert_with(|| {
                self.bd
                    .import_signature(Hooks::try_link_sig(self.consts.ptr_type))
            });

        self.bd.ins().call_indirect(
            try_link_sig,
            try_link_hook,
            &[self.consts.ctx_ptr, destination, link_storage_ptr],
        );

        // was the link successful?
        let stored_link = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            link_storage_ptr,
            0,
        );

        let link_success = self.bd.create_block();
        let link_failure = self.bd.create_block();

        self.bd
            .ins()
            .brif(stored_link, link_success, &[], link_failure, &[]);

        self.bd.seal_block(link_success);
        self.bd.seal_block(link_failure);
        self.bd.set_cold_block(link_failure);

        // => follow link => need to link => success
        self.bd.switch_to_block(link_success);
        self.bd.ins().return_call_indirect(
            self.consts.block_sig,
            stored_link,
            &[
                self.consts.info_ptr,
                self.consts.ctx_ptr,
                self.consts.hooks_ptr,
            ],
        );

        // => follow link => need to link => failure
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

        if block_link {
            self.jump_with_block_link(destination);
        } else {
            self.set(Reg::PC, destination);
            self.flush();
            self.prologue();
        }
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
