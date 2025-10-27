use super::BlockBuilder;
use crate::{
    block::Hooks,
    builder::{Action, Info},
};
use common::arch::{Exception, GPR, InsExt, Reg, SPR, disasm::Ins};
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

impl ReadWriteAble for i64 {
    const IR_TYPE: ir::Type = ir::types::I64;
    const READ_OFFSET: i32 = offset_of!(Hooks, read_i64) as i32;
    const WRITE_OFFSET: i32 = offset_of!(Hooks, write_i64) as i32;
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

        let t = self.ir_value(true);
        self.bd.set_cold_block(exit_block);
        self.bd.ins().brif(t, continue_block, &[], exit_block, &[]);

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

    /// Reads a quantized value. Returns the value and the type size.
    fn read_quantized(&mut self, addr: ir::Value, gqr: ir::Value) -> (ir::Value, ir::Value) {
        let read_quantized_offset = offset_of!(Hooks, read_quantized) as i32;
        let read_fn = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            self.consts.hooks_ptr,
            read_quantized_offset,
        );

        let sig = *self
            .consts
            .hooks_sig
            .entry(read_quantized_offset)
            .or_insert_with(|| {
                self.bd
                    .import_signature(Hooks::read_quantized_sig(self.consts.ptr_type))
            });

        let stack_slot = self.bd.create_sized_stack_slot(ir::StackSlotData::new(
            ir::StackSlotKind::ExplicitSlot,
            size_of::<f64>() as u32,
            align_of::<f64>().ilog2() as u8,
        ));

        let stack_slot_addr = self
            .bd
            .ins()
            .stack_addr(self.consts.ptr_type, stack_slot, 0);

        // NOTE: maybe flush to ensure GQRs are up to date?
        let inst = self.bd.ins().call_indirect(
            sig,
            read_fn,
            &[self.consts.ctx_ptr, addr, gqr, stack_slot_addr],
        );

        let size = self.bd.inst_results(inst)[0];
        let exit_block = self.bd.create_block();
        let continue_block = self.bd.create_block();

        self.bd.set_cold_block(exit_block);
        self.bd
            .ins()
            .brif(size, continue_block, &[], exit_block, &[]);

        self.bd.seal_block(exit_block);
        self.bd.seal_block(continue_block);

        self.switch_to_bb(exit_block);
        self.raise_exception(Exception::DSI);
        self.prologue_with(LOAD_INFO);

        self.switch_to_bb(continue_block);
        (
            self.bd.ins().stack_load(ir::types::F64, stack_slot, 0),
            self.bd.ins().uextend(ir::types::I32, size),
        )
    }

    /// Writes a quantized value. Returns the type size.
    fn write_quantized(&mut self, addr: ir::Value, gqr: ir::Value, value: ir::Value) -> ir::Value {
        let write_quantized_offset = offset_of!(Hooks, write_quantized) as i32;
        let write_fn = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            self.consts.hooks_ptr,
            write_quantized_offset,
        );

        let sig = *self
            .consts
            .hooks_sig
            .entry(write_quantized_offset)
            .or_insert_with(|| {
                self.bd
                    .import_signature(Hooks::write_quantized_sig(self.consts.ptr_type))
            });

        // NOTE: maybe flush to ensure GQRs are up to date?
        let inst =
            self.bd
                .ins()
                .call_indirect(sig, write_fn, &[self.consts.ctx_ptr, addr, gqr, value]);

        let size = self.bd.inst_results(inst)[0];
        let exit_block = self.bd.create_block();
        let continue_block = self.bd.create_block();

        self.bd.set_cold_block(exit_block);
        self.bd
            .ins()
            .brif(size, continue_block, &[], exit_block, &[]);

        self.bd.seal_block(exit_block);
        self.bd.seal_block(continue_block);

        self.switch_to_bb(exit_block);
        self.raise_exception(Exception::DSI);
        self.prologue_with(STORE_INFO);

        self.switch_to_bb(continue_block);
        self.bd.ins().uextend(ir::types::I32, size)
    }
}

const LOAD_INFO: Info = Info {
    cycles: 2,
    auto_pc: true,
    action: Action::Continue,
};

#[derive(Clone, Copy)]
struct LoadOp {
    update: bool,
    signed: bool,
    reverse: bool,
}

/// GPR load operations
impl BlockBuilder<'_> {
    fn load<P: ReadWriteAble>(&mut self, ins: Ins, op: LoadOp) -> Info {
        let addr = if !op.update && ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
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

    fn load_indexed<P: ReadWriteAble>(&mut self, ins: Ins, op: LoadOp) -> Info {
        let rb = self.get(ins.gpr_b());
        let addr = if !op.update && ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let mut value = self.read::<P>(addr);

        if op.reverse {
            value = self.bd.ins().bswap(value);
        }

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
                reverse: false,
            },
        )
    }

    pub fn lbzx(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i8>(
            ins,
            LoadOp {
                update: false,
                signed: false,
                reverse: false,
            },
        )
    }

    pub fn lbzu(&mut self, ins: Ins) -> Info {
        self.load::<i8>(
            ins,
            LoadOp {
                update: true,
                signed: false,
                reverse: false,
            },
        )
    }

    pub fn lbzux(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i8>(
            ins,
            LoadOp {
                update: true,
                signed: false,
                reverse: false,
            },
        )
    }

    pub fn lhz(&mut self, ins: Ins) -> Info {
        self.load::<i16>(
            ins,
            LoadOp {
                update: false,
                signed: false,
                reverse: false,
            },
        )
    }

    pub fn lhzx(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i16>(
            ins,
            LoadOp {
                update: false,
                signed: false,
                reverse: false,
            },
        )
    }

    pub fn lhzu(&mut self, ins: Ins) -> Info {
        self.load::<i16>(
            ins,
            LoadOp {
                update: true,
                signed: false,
                reverse: false,
            },
        )
    }

    pub fn lhzux(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i16>(
            ins,
            LoadOp {
                update: true,
                signed: false,
                reverse: false,
            },
        )
    }

    pub fn lha(&mut self, ins: Ins) -> Info {
        self.load::<i16>(
            ins,
            LoadOp {
                update: false,
                signed: true,
                reverse: false,
            },
        )
    }

    pub fn lhax(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i16>(
            ins,
            LoadOp {
                update: false,
                signed: true,
                reverse: false,
            },
        )
    }

    pub fn lhau(&mut self, ins: Ins) -> Info {
        self.load::<i16>(
            ins,
            LoadOp {
                update: true,
                signed: true,
                reverse: false,
            },
        )
    }

    pub fn lhaux(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i16>(
            ins,
            LoadOp {
                update: true,
                signed: true,
                reverse: false,
            },
        )
    }

    pub fn lhbrx(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i16>(
            ins,
            LoadOp {
                update: false,
                signed: false,
                reverse: true,
            },
        )
    }

    pub fn lwz(&mut self, ins: Ins) -> Info {
        self.load::<i32>(
            ins,
            LoadOp {
                update: false,
                signed: false,
                reverse: false,
            },
        )
    }

    pub fn lwzx(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i32>(
            ins,
            LoadOp {
                update: false,
                signed: false,
                reverse: false,
            },
        )
    }

    pub fn lwzu(&mut self, ins: Ins) -> Info {
        self.load::<i32>(
            ins,
            LoadOp {
                update: true,
                signed: false,
                reverse: false,
            },
        )
    }

    pub fn lwzux(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i32>(
            ins,
            LoadOp {
                update: true,
                signed: false,
                reverse: false,
            },
        )
    }

    pub fn lwbrx(&mut self, ins: Ins) -> Info {
        self.load_indexed::<i32>(
            ins,
            LoadOp {
                update: false,
                signed: false,
                reverse: true,
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

    pub fn lswi(&mut self, ins: Ins) -> Info {
        let mut addr = if ins.field_ra() == 0 {
            self.ir_value(0)
        } else {
            self.get(ins.gpr_a())
        };

        let byte_count = if ins.field_nb() != 0 {
            ins.field_nb()
        } else {
            32
        };

        let zero = self.ir_value(0);
        let start_reg = ins.field_rd();
        for i in 0..byte_count {
            let reg = GPR::new((start_reg + i / 4) % 32);
            let shift_count = 8 * (3 - (i as u32 % 4));

            let value = self.read::<i8>(addr);
            let value = self.bd.ins().uextend(ir::types::I32, value);
            let value = self.bd.ins().ishl_imm(value, shift_count as u64 as i64);

            let current = self.get(reg);
            let mask = self.ir_value(0xFFu32 << shift_count);
            let loaded = self.bd.ins().bitselect(mask, value, current);

            let clear_mask = self.ir_value(0xFFFF_FFFFu32 << shift_count);
            let new = self.bd.ins().bitselect(clear_mask, loaded, zero);

            self.set(reg, new);
            addr = self.bd.ins().iadd_imm(addr, 1);
        }

        Info {
            cycles: 10, // random, chosen by fair dice roll
            ..LOAD_INFO
        }
    }

    pub fn lfd(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.read::<i64>(addr);
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::F64, ir::MemFlags::new(), value);

        let paired = self.bd.ins().splat(ir::types::F64X2, value);
        self.set_ps(ins.fpr_d(), paired);

        LOAD_INFO
    }

    pub fn lfdu(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.read::<i64>(addr);
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::F64, ir::MemFlags::new(), value);

        let paired = self.bd.ins().splat(ir::types::F64X2, value);
        self.set_ps(ins.fpr_d(), paired);
        self.set(ins.gpr_a(), addr);

        LOAD_INFO
    }

    pub fn lfdx(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let rb = self.get(ins.gpr_b());
        let addr = if ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let value = self.read::<i64>(addr);
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::F64, ir::MemFlags::new(), value);

        let paired = self.bd.ins().splat(ir::types::F64X2, value);
        self.set_ps(ins.fpr_d(), paired);

        LOAD_INFO
    }

    pub fn lfdux(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());
        let addr = self.bd.ins().iadd(ra, rb);

        let value = self.read::<i64>(addr);
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::F64, ir::MemFlags::new(), value);

        let paired = self.bd.ins().splat(ir::types::F64X2, value);
        self.set_ps(ins.fpr_d(), paired);
        self.set(ins.gpr_a(), addr);

        LOAD_INFO
    }

    pub fn lfs(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.read::<i32>(addr);
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::F32, ir::MemFlags::new(), value);

        let double = self.bd.ins().fpromote(ir::types::F64, value);
        let paired = self.bd.ins().splat(ir::types::F64X2, double);
        self.set_ps(ins.fpr_d(), paired);

        LOAD_INFO
    }

    pub fn lfsu(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.read::<i32>(addr);
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::F32, ir::MemFlags::new(), value);

        let double = self.bd.ins().fpromote(ir::types::F64, value);
        let paired = self.bd.ins().splat(ir::types::F64X2, double);
        self.set_ps(ins.fpr_d(), paired);
        self.set(ins.gpr_a(), addr);

        LOAD_INFO
    }

    pub fn lfsx(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let rb = self.get(ins.gpr_b());
        let addr = if ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let value = self.read::<i32>(addr);
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::F32, ir::MemFlags::new(), value);

        let double = self.bd.ins().fpromote(ir::types::F64, value);
        let paired = self.bd.ins().splat(ir::types::F64X2, double);
        self.set_ps(ins.fpr_d(), paired);

        LOAD_INFO
    }

    pub fn lfsux(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let ra = self.get(ins.gpr_a());
        let rb = self.get(ins.gpr_b());
        let addr = self.bd.ins().iadd(ra, rb);

        let value = self.read::<i32>(addr);
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::F32, ir::MemFlags::new(), value);

        let double = self.bd.ins().fpromote(ir::types::F64, value);
        let paired = self.bd.ins().splat(ir::types::F64X2, double);
        self.set_ps(ins.fpr_d(), paired);
        self.set(ins.gpr_a(), addr);

        LOAD_INFO
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

    fn store_indexed<P: ReadWriteAble>(&mut self, ins: Ins, update: bool, reverse: bool) -> Info {
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

        if reverse {
            value = self.bd.ins().bswap(value);
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
        self.store_indexed::<i8>(ins, false, false)
    }

    pub fn stbu(&mut self, ins: Ins) -> Info {
        self.store::<i8>(ins, true)
    }

    pub fn stbux(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i8>(ins, true, false)
    }

    pub fn sth(&mut self, ins: Ins) -> Info {
        self.store::<i16>(ins, false)
    }

    pub fn sthx(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i16>(ins, false, false)
    }

    pub fn sthbrx(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i16>(ins, false, true)
    }

    pub fn sthu(&mut self, ins: Ins) -> Info {
        self.store::<i16>(ins, true)
    }

    pub fn sthux(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i16>(ins, true, false)
    }

    pub fn stw(&mut self, ins: Ins) -> Info {
        self.store::<i32>(ins, false)
    }

    pub fn stwx(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i32>(ins, false, false)
    }

    pub fn stwbrx(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i32>(ins, false, true)
    }

    pub fn stwu(&mut self, ins: Ins) -> Info {
        self.store::<i32>(ins, true)
    }

    pub fn stwux(&mut self, ins: Ins) -> Info {
        self.store_indexed::<i32>(ins, true, false)
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

    pub fn stswi(&mut self, ins: Ins) -> Info {
        let mut addr = if ins.field_ra() == 0 {
            self.ir_value(0)
        } else {
            self.get(ins.gpr_a())
        };

        let byte_count = if ins.field_nb() != 0 {
            ins.field_nb()
        } else {
            32
        };

        let start_reg = ins.field_rd();
        for i in 0..byte_count {
            let reg = GPR::new((start_reg + i / 4) % 32);
            let shift_count = 8 * (3 - (i as u32 % 4));

            let reg = self.get(reg);
            let value = self.bd.ins().ushr_imm(reg, shift_count as u64 as i64);
            let value = self.bd.ins().ireduce(ir::types::I8, value);

            self.write::<i8>(addr, value);
            addr = self.bd.ins().iadd_imm(addr, 1);
        }

        Info {
            cycles: 10, // random, chosen by fair dice roll
            ..LOAD_INFO
        }
    }

    pub fn stfd(&mut self, ins: Ins) -> Info {
        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.get(ins.fpr_s());
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::I64, ir::MemFlags::new(), value);

        self.write::<i64>(addr, value);

        STORE_INFO
    }

    pub fn stfdu(&mut self, ins: Ins) -> Info {
        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.get(ins.fpr_s());
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::I64, ir::MemFlags::new(), value);

        self.write::<i64>(addr, value);
        self.set(ins.gpr_a(), addr);

        STORE_INFO
    }

    pub fn stfs(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.get(ins.fpr_s());
        let value = self.bd.ins().fdemote(ir::types::F32, value);
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::I32, ir::MemFlags::new(), value);

        self.write::<i32>(addr, value);

        STORE_INFO
    }

    pub fn stfsx(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let rb = self.get(ins.gpr_b());
        let addr = if ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let value = self.get(ins.fpr_s());
        let value = self.bd.ins().fdemote(ir::types::F32, value);
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::I32, ir::MemFlags::new(), value);

        self.write::<i32>(addr, value);

        STORE_INFO
    }

    pub fn stfsu(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.get(ins.fpr_s());
        let value = self.bd.ins().fdemote(ir::types::F32, value);
        let value = self
            .bd
            .ins()
            .bitcast(ir::types::I32, ir::MemFlags::new(), value);

        self.write::<i32>(addr, value);
        self.set(ins.gpr_a(), addr);

        STORE_INFO
    }
}

/// FPR store operations
impl BlockBuilder<'_> {
    pub fn stfiwx(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let rb = self.get(ins.gpr_b());
        let addr = if ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let fpr_s = self.get(ins.fpr_s());
        let int64 = self
            .bd
            .ins()
            .bitcast(ir::types::I64, ir::MemFlags::new(), fpr_s);
        let int32 = self.bd.ins().ireduce(ir::types::I32, int64);

        self.write::<i32>(addr, int32);

        STORE_INFO
    }

    pub fn psq_l(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_ps_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_ps_offset() as i64)
        };

        let index = self.ir_value(ins.field_ps_i());
        let (ps0, size) = self.read_quantized(addr, index);
        let ps1 = if ins.field_ps_w() == 0 {
            let addr = self.bd.ins().iadd(addr, size);
            self.read_quantized(addr, index).0
        } else {
            self.ir_value(1.0f64)
        };

        let fpr_d = ins.fpr_d();
        self.set(fpr_d, ps0);
        self.set(Reg::PS1(fpr_d), ps1);

        LOAD_INFO
    }

    pub fn psq_st(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_ps_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_ps_offset() as i64)
        };

        let ps0 = self.get(ins.fpr_s());
        let ps1 = self.get(Reg::PS1(ins.fpr_s()));
        let index = self.ir_value(ins.field_ps_i());

        let size = self.write_quantized(addr, index, ps0);
        if ins.field_ps_w() == 0 {
            let addr = self.bd.ins().iadd(addr, size);
            self.write_quantized(addr, index, ps1);
        }

        STORE_INFO
    }
}
