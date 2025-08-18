use super::BlockBuilder;
use crate::{block::Functions, builder::Reg};
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, isa::CallConv},
};
use powerpc::Ins;
use std::mem::offset_of;

fn read_signature(ptr_type: ir::Type, read_type: ir::Type) -> ir::Signature {
    ir::Signature {
        params: vec![
            ir::AbiParam::new(ptr_type),       // bus
            ir::AbiParam::new(ptr_type),       // regs
            ir::AbiParam::new(ir::types::I32), // address
        ],
        returns: vec![ir::AbiParam::new(read_type)], // value
        call_conv: CallConv::SystemV,
    }
}

fn write_signature(ptr_type: ir::Type, write_type: ir::Type) -> ir::Signature {
    ir::Signature {
        params: vec![
            ir::AbiParam::new(ptr_type),       // bus
            ir::AbiParam::new(ptr_type),       // regs
            ir::AbiParam::new(ir::types::I32), // address
            ir::AbiParam::new(write_type),     // value
        ],
        returns: vec![],
        call_conv: CallConv::SystemV,
    }
}

impl BlockBuilder<'_> {
    pub fn stwu(&mut self, ins: Ins) {
        let base = self.get(Reg::Gpr(ins.field_ra()));
        let addr = self.bd.ins().iadd_imm(base, ins.field_offset() as i64);

        let bus = self.bd.ins().load(
            self.ctx.ptr_type,
            ir::MemFlags::trusted(),
            self.ctx.functions_ptr,
            offset_of!(Functions, bus) as i32,
        );
        let to_call = self.bd.ins().load(
            self.ctx.ptr_type,
            ir::MemFlags::trusted(),
            self.ctx.functions_ptr,
            offset_of!(Functions, write_i32) as i32,
        );

        let sig = self
            .bd
            .import_signature(write_signature(self.ctx.ptr_type, ir::types::I32));

        let value = self.get(Reg::Gpr(ins.field_rs()));
        self.bd
            .ins()
            .call_indirect(sig, to_call, &[bus, self.ctx.regs_ptr, addr, value]);
    }
}
