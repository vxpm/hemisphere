use super::BlockBuilder;
use crate::block::Hooks;
use common::arch::{GPR, InsExt, disasm::Ins};
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

        let inst = self
            .bd
            .ins()
            .call_indirect(sig, read_fn, &[self.consts.ctx_ptr, addr]);

        let ret = self.bd.inst_results(inst);
        ret[0]
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

    pub fn stwu(&mut self, ins: Ins) {
        let value = self.get(ins.gpr_s());
        let base = self.get(ins.gpr_a());
        let addr = self.bd.ins().iadd_imm(base, ins.field_offset() as i64);
        self.set(ins.gpr_a(), addr);
        self.write::<i32>(addr, value);
    }

    pub fn stw(&mut self, ins: Ins) {
        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.get(ins.gpr_s());
        self.write::<i32>(addr, value);
    }

    pub fn stmw(&mut self, ins: Ins) {
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
    }

    pub fn stwx(&mut self, ins: Ins) {
        let rb = self.get(ins.gpr_b());
        let addr = if ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let value = self.get(ins.gpr_s());
        self.write::<i32>(addr, value);
    }

    pub fn sth(&mut self, ins: Ins) {
        let value = self.get(ins.gpr_s());
        let value = self.bd.ins().ireduce(ir::types::I16, value);

        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        self.write::<i16>(addr, value);
    }

    pub fn sthu(&mut self, ins: Ins) {
        let value = self.get(ins.gpr_s());
        let value = self.bd.ins().ireduce(ir::types::I16, value);
        let base = self.get(ins.gpr_a());
        let addr = self.bd.ins().iadd_imm(base, ins.field_offset() as i64);
        self.set(ins.gpr_a(), addr);
        self.write::<i16>(addr, value);
    }

    pub fn sthx(&mut self, ins: Ins) {
        let rb = self.get(ins.gpr_b());
        let addr = if ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let value = self.get(ins.gpr_s());
        let extended = self.bd.ins().ireduce(ir::types::I16, value);
        self.write::<i16>(addr, extended);
    }

    pub fn stb(&mut self, ins: Ins) {
        let value = self.get(ins.gpr_s());
        let value = self.bd.ins().ireduce(ir::types::I8, value);

        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        self.write::<i8>(addr, value);
    }

    pub fn stbu(&mut self, ins: Ins) {
        let value = self.get(ins.gpr_s());
        let value = self.bd.ins().ireduce(ir::types::I8, value);

        let base = self.get(ins.gpr_a());
        let addr = self.bd.ins().iadd_imm(base, ins.field_offset() as i64);
        self.set(ins.gpr_a(), addr);
        self.write::<i8>(addr, value);
    }

    pub fn stbx(&mut self, ins: Ins) {
        let rb = self.get(ins.gpr_b());
        let addr = if ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let value = self.get(ins.gpr_s());
        let extended = self.bd.ins().ireduce(ir::types::I8, value);
        self.write::<i8>(addr, extended);
    }

    pub fn lmw(&mut self, ins: Ins) {
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
    }

    pub fn lwz(&mut self, ins: Ins) {
        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.read::<i32>(addr);
        self.set(ins.gpr_d(), value);
    }

    pub fn lwzx(&mut self, ins: Ins) {
        let rb = self.get(ins.gpr_b());
        let addr = if ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let value = self.read::<i32>(addr);
        self.set(ins.gpr_d(), value);
    }

    pub fn lwzu(&mut self, ins: Ins) {
        let base = self.get(ins.gpr_a());
        let addr = self.bd.ins().iadd_imm(base, ins.field_offset() as i64);
        let value = self.read::<i32>(addr);
        self.set(ins.gpr_d(), value);
        self.set(ins.gpr_a(), addr);
    }

    pub fn lhz(&mut self, ins: Ins) {
        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.read::<i16>(addr);
        let extended = self.bd.ins().uextend(ir::types::I32, value);

        self.set(ins.gpr_d(), extended);
    }

    pub fn lha(&mut self, ins: Ins) {
        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.read::<i16>(addr);
        let extended = self.bd.ins().sextend(ir::types::I32, value);

        self.set(ins.gpr_d(), extended);
    }

    pub fn lhzx(&mut self, ins: Ins) {
        let rb = self.get(ins.gpr_b());
        let addr = if ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let value = self.read::<i16>(addr);
        let extended = self.bd.ins().uextend(ir::types::I32, value);

        self.set(ins.gpr_d(), extended);
    }

    pub fn lbz(&mut self, ins: Ins) {
        let addr = if ins.field_ra() == 0 {
            self.ir_value(ins.field_offset() as i32)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.read::<i8>(addr);
        let extended = self.bd.ins().uextend(ir::types::I32, value);

        self.set(ins.gpr_d(), extended);
    }

    pub fn lbzu(&mut self, ins: Ins) {
        let base = self.get(ins.gpr_a());
        let addr = self.bd.ins().iadd_imm(base, ins.field_offset() as i64);
        let value = self.read::<i8>(addr);
        let extended = self.bd.ins().uextend(ir::types::I32, value);
        self.set(ins.gpr_d(), extended);
        self.set(ins.gpr_a(), addr);
    }

    pub fn lbzx(&mut self, ins: Ins) {
        let rb = self.get(ins.gpr_b());
        let addr = if ins.field_ra() == 0 {
            rb
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd(ra, rb)
        };

        let value = self.read::<i8>(addr);
        let extended = self.bd.ins().uextend(ir::types::I32, value);

        self.set(ins.gpr_d(), extended);
    }
}
