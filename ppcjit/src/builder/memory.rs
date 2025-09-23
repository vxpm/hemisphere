use super::BlockBuilder;
use crate::{
    block::Hooks,
    builder::{Action, Info},
};
use common::arch::{Exception, GPR, InsExt, disasm::Ins};
use cranelift::{codegen::ir, prelude::InstBuilder};
use std::mem::offset_of;

trait ReadWriteAble {
    const IR_TYPE: ir::Type;
    const READ_OFFSET: i32;
    const WRITE_OFFSET: i32;
}

impl ReadWriteAble for i8 {
    const IR_TYPE: ir::Type = ir::types::I8;
    const READ_OFFSET: i32 = offset_of!(Hooks, read_i8) as i32;
    const WRITE_OFFSET: i32 = offset_of!(Hooks, write_i8) as i32;
}

impl ReadWriteAble for i16 {
    const IR_TYPE: ir::Type = ir::types::I16;
    const READ_OFFSET: i32 = offset_of!(Hooks, read_i16) as i32;
    const WRITE_OFFSET: i32 = offset_of!(Hooks, write_i16) as i32;
}

impl ReadWriteAble for i32 {
    const IR_TYPE: ir::Type = ir::types::I32;
    const READ_OFFSET: i32 = offset_of!(Hooks, read_i32) as i32;
    const WRITE_OFFSET: i32 = offset_of!(Hooks, write_i32) as i32;
}

/// Helpers
impl BlockBuilder<'_> {
    fn read<P: ReadWriteAble>(&mut self, addr: ir::Value) -> ir::Value {
        let read_fn = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            self.consts.hooks_ptr,
            P::READ_OFFSET,
        );

        let sig = *self
            .consts
            .hooks_sig
            .entry(P::READ_OFFSET)
            .or_insert_with(|| {
                self.bd
                    .import_signature(Hooks::read_sig(self.consts.ptr_type, P::IR_TYPE))
            });

        let stack_slot = self.bd.create_sized_stack_slot(ir::StackSlotData::new(
            ir::StackSlotKind::ExplicitSlot,
            size_of::<P>() as u32,
            align_of::<P>().ilog2() as u8,
        ));

        let stack_slot_addr = self
            .bd
            .ins()
            .stack_addr(self.consts.ptr_type, stack_slot, 0);

        let inst = self.bd.ins().call_indirect(
            sig,
            read_fn,
            &[self.consts.ctx_ptr, addr, stack_slot_addr],
        );

        let success = self.bd.inst_results(inst)[0];
        let exit_block = self.bd.create_block();
        let continue_block = self.bd.create_block();

        self.bd.set_cold_block(exit_block);
        self.bd
            .ins()
            .brif(success, continue_block, &[], exit_block, &[]);

        self.bd.seal_block(exit_block);
        self.bd.seal_block(continue_block);

        self.switch_to_bb(exit_block);
        self.raise_exception(Exception::DSI);
        self.prologue_with(LOAD_INFO);

        self.switch_to_bb(continue_block);
        self.bd.ins().stack_load(P::IR_TYPE, stack_slot, 0)
    }

    fn write<P: ReadWriteAble>(&mut self, addr: ir::Value, value: ir::Value) {
        let write_fn = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            self.consts.hooks_ptr,
            P::WRITE_OFFSET,
        );

        let sig = *self
            .consts
            .hooks_sig
            .entry(P::WRITE_OFFSET)
            .or_insert_with(|| {
                self.bd
                    .import_signature(Hooks::write_sig(self.consts.ptr_type, P::IR_TYPE))
            });

        self.bd
            .ins()
            .call_indirect(sig, write_fn, &[self.consts.ctx_ptr, addr, value]);
    }
}

const LOAD_INFO: Info = Info {
    cycles: 2,
    auto_pc: true,
    action: Action::Continue,
};

struct LoadOp {
    update: bool,
    signed: bool,
}

/// Load operations
impl BlockBuilder<'_> {
    fn load<P: ReadWriteAble>(&mut self, ins: Ins, op: LoadOp) -> Info {
        let addr = if !op.update && ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        self.flush();
        let mut value = self.read::<P>(addr);
        if P::IR_TYPE != ir::types::I32 {
            value = if op.signed {
                self.bd.ins().sextend(ir::types::I32, value)
            } else {
                self.bd.ins().uextend(ir::types::I32, value)
            };
        }

        if op.update {
            self.set(ins.gpr_a(), addr);
        }

        self.set(ins.gpr_d(), value);

        LOAD_INFO
    }

    fn load_indexed<P: ReadWriteAble>(&mut self, ins: Ins, op: LoadOp) -> Info {
        let rb = self.get(ins.gpr_b());
        let addr = if !op.update && ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let mut value = self.read::<P>(addr);
        if P::IR_TYPE != ir::types::I32 {
            value = if op.signed {
                self.bd.ins().sextend(ir::types::I32, value)
            } else {
                self.bd.ins().uextend(ir::types::I32, value)
            };
        }

        if op.update {
            self.set(ins.gpr_a(), addr);
        }

        self.set(ins.gpr_d(), value);

        LOAD_INFO
    }

    pub fn lbz(&mut self, ins: Ins) -> Info {
        self.load::<i8>(
            ins,
            LoadOp {
                update: false,
                signed: false,
            },
        )
    }

    pub fn lbzx(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i8>(
            ins,
            LoadOp {
                update: false,
                signed: false,
            },
        )
    }

    pub fn lbzu(&mut self, ins: Ins) -> Info {
        self.load::<i8>(
            ins,
            LoadOp {
                update: true,
                signed: false,
            },
        )
    }

    pub fn lbzux(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i8>(
            ins,
            LoadOp {
                update: true,
                signed: false,
            },
        )
    }

    pub fn lhz(&mut self, ins: Ins) -> Info {
        self.load::<i16>(
            ins,
            LoadOp {
                update: false,
                signed: false,
            },
        )
    }

    pub fn lhzx(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i16>(
            ins,
            LoadOp {
                update: false,
                signed: false,
            },
        )
    }

    pub fn lhzu(&mut self, ins: Ins) -> Info {
        self.load::<i16>(
            ins,
            LoadOp {
                update: true,
                signed: false,
            },
        )
    }

    pub fn lhzux(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i16>(
            ins,
            LoadOp {
                update: true,
                signed: false,
            },
        )
    }

    pub fn lha(&mut self, ins: Ins) -> Info {
        self.load::<i16>(
            ins,
            LoadOp {
                update: false,
                signed: true,
            },
        )
    }

    pub fn lhax(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i16>(
            ins,
            LoadOp {
                update: false,
                signed: true,
            },
        )
    }

    pub fn lhau(&mut self, ins: Ins) -> Info {
        self.load::<i16>(
            ins,
            LoadOp {
                update: true,
                signed: true,
            },
        )
    }

    pub fn lhaux(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i16>(
            ins,
            LoadOp {
                update: true,
                signed: true,
            },
        )
    }

    pub fn lwz(&mut self, ins: Ins) -> Info {
        self.load::<i32>(
            ins,
            LoadOp {
                update: false,
                signed: false,
            },
        )
    }

    pub fn lwzx(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i32>(
            ins,
            LoadOp {
                update: false,
                signed: false,
            },
        )
    }

    pub fn lwzu(&mut self, ins: Ins) -> Info {
        self.load::<i32>(
            ins,
            LoadOp {
                update: true,
                signed: false,
            },
        )
    }

    pub fn lwzux(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i32>(
            ins,
            LoadOp {
                update: true,
                signed: false,
            },
        )
    }

    pub fn lmw(&mut self, ins: Ins) -> Info {
        let mut addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        for i in ins.field_rd()..32 {
            let value = self.read::<i32>(addr);
            self.set(GPR::new(i), value);

            addr = self.bd.ins().iadd_imm(addr, 4);
        }

        Info {
            cycles: 10, // random, chosen by fair dice roll
            ..LOAD_INFO
        }
    }
}

const STORE_INFO: Info = Info {
    cycles: 2,
    auto_pc: true,
    action: Action::Continue,
};

/// Store operations
impl BlockBuilder<'_> {
    fn store<P: ReadWriteAble>(&mut self, ins: Ins, update: bool) -> Info {
        let addr = if !update && ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let mut value = self.get(ins.gpr_s());
        if P::IR_TYPE != ir::types::I32 {
            value = self.bd.ins().ireduce(P::IR_TYPE, value);
        }

        if update {
            self.set(ins.gpr_a(), addr);
        }

        self.write::<P>(addr, value);

        STORE_INFO
    }

    fn store_indexed<P: ReadWriteAble>(&mut self, ins: Ins, update: bool) -> Info {
        let rb = self.get(ins.gpr_b());
        let addr = if !update && ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let mut value = self.get(ins.gpr_s());
        if P::IR_TYPE != ir::types::I32 {
            value = self.bd.ins().ireduce(P::IR_TYPE, value);
        }

        if update {
            self.set(ins.gpr_a(), addr);
        }

        self.write::<P>(addr, value);

        STORE_INFO
    }

    pub fn stb(&mut self, ins: Ins) -> Info {
        self.store::<i8>(ins, false)
    }

    pub fn stbx(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i8>(ins, false)
    }

    pub fn stbu(&mut self, ins: Ins) -> Info {
        self.store::<i8>(ins, true)
    }

    pub fn stbux(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i8>(ins, true)
    }

    pub fn sth(&mut self, ins: Ins) -> Info {
        self.store::<i16>(ins, false)
    }

    pub fn sthx(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i16>(ins, false)
    }

    pub fn sthu(&mut self, ins: Ins) -> Info {
        self.store::<i16>(ins, true)
    }

    pub fn sthux(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i8>(ins, true)
    }

    pub fn stw(&mut self, ins: Ins) -> Info {
        self.store::<i32>(ins, false)
    }

    pub fn stwx(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i32>(ins, false)
    }

    pub fn stwu(&mut self, ins: Ins) -> Info {
        self.store::<i32>(ins, true)
    }

    pub fn stwux(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i32>(ins, true)
    }

    pub fn stmw(&mut self, ins: Ins) -> Info {
        let mut addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        for i in ins.field_rs()..32 {
            let value = self.get(GPR::new(i));
            self.write::<i32>(addr, value);

            addr = self.bd.ins().iadd_imm(addr, 4);
        }

        Info {
            cycles: 10, // random, chosen by fair dice roll
            ..STORE_INFO
        }
    }
}
