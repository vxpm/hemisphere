use super::BlockBuilder;
use crate::{block::ExternalFunctions, builder::Reg};
use cranelift::{
    codegen::ir,
    prelude::{InstBuilder, isa::CallConv},
};
use powerpc::Ins;
use std::mem::offset_of;

fn sig_read(ptr_type: ir::Type, read_type: ir::Type) -> ir::Signature {
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

fn sig_write(ptr_type: ir::Type, write_type: ir::Type) -> ir::Signature {
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

trait IrType {
    const READ_OFFSET: i32;
    const WRITE_OFFSET: i32;
    const IR_TYPE: ir::Type;
}

impl IrType for i32 {
    const READ_OFFSET: i32 = offset_of!(ExternalFunctions, read_i32) as i32;
    const WRITE_OFFSET: i32 = offset_of!(ExternalFunctions, write_i32) as i32;
    const IR_TYPE: ir::Type = ir::types::I32;
}

impl IrType for i16 {
    const READ_OFFSET: i32 = offset_of!(ExternalFunctions, read_i16) as i32;
    const WRITE_OFFSET: i32 = offset_of!(ExternalFunctions, write_i16) as i32;
    const IR_TYPE: ir::Type = ir::types::I16;
}

impl BlockBuilder<'_> {
    fn write<P: IrType>(&mut self, addr: ir::Value, value: ir::Value) {
        let write_fn = self.bd.ins().load(
            self.ctx.ptr_type,
            ir::MemFlags::trusted(),
            self.ctx.external_functions_ptr,
            P::WRITE_OFFSET,
        );

        let sig = self
            .bd
            .import_signature(sig_write(self.ctx.ptr_type, P::IR_TYPE));

        self.bd.ins().call_indirect(
            sig,
            write_fn,
            &[self.ctx.external_data_ptr, self.ctx.regs_ptr, addr, value],
        );
    }

    pub fn stwu(&mut self, ins: Ins) {
        let value = self.get(Reg::Gpr(ins.field_rs()));
        let base = self.get(Reg::Gpr(ins.field_ra()));
        let addr = self.bd.ins().iadd_imm(base, ins.field_offset() as i64);
        self.set(Reg::Gpr(ins.field_ra()), addr);
        self.write::<i32>(addr, value);
    }

    pub fn sth(&mut self, ins: Ins) {
        let value = self.get(Reg::Gpr(ins.field_rs()));
        let value = self.bd.ins().ireduce(ir::types::I16, value);

        let addr = if ins.field_ra() == 0 {
            self.bd
                .ins()
                .iconst(ir::types::I32, ins.field_offset() as i64)
        } else {
            let ra = self.get(Reg::Gpr(ins.field_ra()));
            self.bd.ins().iadd_imm(ra, ins.field_offset() as i64)
        };

        self.write::<i16>(addr, value);
    }
}
