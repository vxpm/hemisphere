use super::BlockBuilder;
use crate::block::ContextHooks;
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, isa::CallConv},
};
use hemicore::arch::{GPR, InsExt, powerpc::Ins};
use std::mem::offset_of;

fn sig_read(ptr_type: ir::Type, read_type: ir::Type) -> ir::Signature {
    ir::Signature {
        params: vec![
            ir::AbiParam::new(ptr_type),       // ctx
            ir::AbiParam::new(ir::types::I32), // address
        ],
        returns: vec![ir::AbiParam::new(read_type)], // value
        call_conv: CallConv::SystemV,
    }
}

fn sig_write(ptr_type: ir::Type, write_type: ir::Type) -> ir::Signature {
    ir::Signature {
        params: vec![
            ir::AbiParam::new(ptr_type),       // ctx
            ir::AbiParam::new(ir::types::I32), // address
            ir::AbiParam::new(write_type),     // value
        ],
        returns: vec![],
        call_conv: CallConv::SystemV,
    }
}

trait IrPrimitive {
    const READ_OFFSET: i32;
    const WRITE_OFFSET: i32;
    const IR_TYPE: ir::Type;
}

impl IrPrimitive for i8 {
    const READ_OFFSET: i32 = offset_of!(ContextHooks, read_i8) as i32;
    const WRITE_OFFSET: i32 = offset_of!(ContextHooks, write_i8) as i32;
    const IR_TYPE: ir::Type = ir::types::I8;
}

impl IrPrimitive for i16 {
    const READ_OFFSET: i32 = offset_of!(ContextHooks, read_i16) as i32;
    const WRITE_OFFSET: i32 = offset_of!(ContextHooks, write_i16) as i32;
    const IR_TYPE: ir::Type = ir::types::I16;
}

impl IrPrimitive for i32 {
    const READ_OFFSET: i32 = offset_of!(ContextHooks, read_i32) as i32;
    const WRITE_OFFSET: i32 = offset_of!(ContextHooks, write_i32) as i32;
    const IR_TYPE: ir::Type = ir::types::I32;
}

impl BlockBuilder<'_> {
    fn read<P: IrPrimitive>(&mut self, addr: ir::Value) -> ir::Value {
        let read_fn = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            self.consts.ctx_hooks_ptr,
            P::READ_OFFSET,
        );

        let sig = *self
            .consts
            .ctx_hooks_sig
            .entry(P::READ_OFFSET)
            .or_insert_with(|| {
                self.bd
                    .import_signature(sig_read(self.consts.ptr_type, P::IR_TYPE))
            });

        let inst = self
            .bd
            .ins()
            .call_indirect(sig, read_fn, &[self.consts.ctx_ptr, addr]);

        let ret = self.bd.inst_results(inst);
        ret[0]
    }

    fn write<P: IrPrimitive>(&mut self, addr: ir::Value, value: ir::Value) {
        let write_fn = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            self.consts.ctx_hooks_ptr,
            P::WRITE_OFFSET,
        );

        let sig = *self
            .consts
            .ctx_hooks_sig
            .entry(P::WRITE_OFFSET)
            .or_insert_with(|| {
                self.bd
                    .import_signature(sig_write(self.consts.ptr_type, P::IR_TYPE))
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
            self.bd
                .ins()
                .iconst(ir::types::I32, ins.field_offset() as i64)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.get(ins.gpr_s());
        self.write::<i32>(addr, value);
    }

    pub fn stmw(&mut self, ins: Ins) {
        let mut addr = if ins.field_ra() == 0 {
            self.bd
                .ins()
                .iconst(ir::types::I32, ins.field_offset() as i64)
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
            self.bd
                .ins()
                .iconst(ir::types::I32, ins.field_offset() as i64)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        self.write::<i16>(addr, value);
    }

    pub fn stb(&mut self, ins: Ins) {
        let value = self.get(ins.gpr_s());
        let value = self.bd.ins().ireduce(ir::types::I8, value);

        let addr = if ins.field_ra() == 0 {
            self.bd
                .ins()
                .iconst(ir::types::I32, ins.field_offset() as i64)
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

    pub fn lmw(&mut self, ins: Ins) {
        let mut addr = if ins.field_ra() == 0 {
            self.bd
                .ins()
                .iconst(ir::types::I32, ins.field_offset() as i64)
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
            self.bd
                .ins()
                .iconst(ir::types::I32, ins.field_offset() as i64)
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

    pub fn lbz(&mut self, ins: Ins) {
        let addr = if ins.field_ra() == 0 {
            self.bd
                .ins()
                .iconst(ir::types::I32, ins.field_offset() as i64)
        } else {
            let ra = self.get(ins.gpr_a());
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        let value = self.read::<i8>(addr);
        let extended = self.bd.ins().uextend(ir::types::I32, value);

        self.set(ins.gpr_d(), extended);
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
