use super::BlockBuilder;
use crate::builder::{Action, Info};
use common::arch::{Cpu, Exception, Reg, SPR, disasm::Ins};
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, IntCC, isa},
};

const RFI_INFO: Info = Info {
    cycles: 2,
    auto_pc: false,
    action: Action::FlushAndPrologue,
};

const EXCEPTION_INFO: Info = Info {
    cycles: 2,
    auto_pc: false,
    action: Action::Prologue,
};

fn raise_exception_sig(ptr_type: ir::Type) -> ir::Signature {
    ir::Signature {
        params: vec![
            ir::AbiParam::new(ptr_type),       // registers
            ir::AbiParam::new(ir::types::I16), // exception
        ],
        returns: vec![],
        call_conv: isa::CallConv::SystemV,
    }
}

extern "sysv64-unwind" fn raise_exception(regs: &mut Cpu, exception: Exception) {
    regs.raise_exception(exception);
}

impl BlockBuilder<'_> {
    /// # Warning
    /// You should _always_ exit after raising an exception.
    pub fn raise_exception(&mut self, exception: Exception) {
        let func = raise_exception as extern "sysv64-unwind" fn(_, _);
        let ptr = self
            .bd
            .ins()
            .iconst(self.consts.ptr_type, func as usize as u64 as i64);
        let sig = *self.consts.raise_exception_sig.get_or_insert_with(|| {
            self.bd
                .import_signature(raise_exception_sig(self.consts.ptr_type))
        });

        let exception = self
            .bd
            .ins()
            .iconst(ir::types::I16, exception as u64 as i64);

        self.flush();

        self.bd
            .ins()
            .call_indirect(sig, ptr, &[self.consts.regs_ptr, exception]);
    }

    pub fn check_floats(&mut self) {
        if self.floats_checked {
            return;
        }
        self.floats_checked = true;

        let msr = self.get(Reg::MSR);
        let fp_enabled = self.get_bit(msr, 13);
        let branch = self.bd.ins().icmp_imm(IntCC::Equal, fp_enabled, 0);

        let exit_block = self.bd.create_block();
        let continue_block = self.bd.create_block();

        self.bd.set_cold_block(exit_block);

        self.bd
            .ins()
            .brif(branch, exit_block, &[], continue_block, &[]);

        self.bd.seal_block(exit_block);
        self.bd.seal_block(continue_block);

        self.switch_to_bb(exit_block);
        self.raise_exception(Exception::Syscall);
        self.prologue_with(EXCEPTION_INFO);

        self.switch_to_bb(continue_block);
        self.current_bb = continue_block;
    }

    pub fn sc(&mut self, _: Ins) -> Info {
        self.raise_exception(Exception::Syscall);
        EXCEPTION_INFO
    }

    pub fn rfi(&mut self, _: Ins) -> Info {
        let msr = self.get(Reg::MSR);
        let srr0 = self.get(SPR::SRR0);
        let srr1 = self.get(SPR::SRR1);
        let mask = self.ir_value(Exception::SRR1_TO_MSR_MASK);

        // move only some bits from srr1
        let new_msr = self.bd.ins().bitselect(mask, srr1, msr);

        // clear bit 18
        let new_msr = self.bd.ins().band_imm(new_msr, !(1 << 18));

        // TODO: deal with new_msr exceptions enabled

        // set PC to SRR0
        let new_pc = self.bd.ins().band_imm(srr0, !0b11);
        self.set(Reg::PC, new_pc);
        self.set(Reg::MSR, new_msr);

        RFI_INFO
    }
}
