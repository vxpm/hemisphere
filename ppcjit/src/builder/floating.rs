use super::BlockBuilder;
use crate::builder::{Action, Info};
use common::arch::{InsExt, disasm::Ins};
use cranelift::{codegen::ir, prelude::InstBuilder};

const FLOAT_INFO: Info = Info {
    cycles: 1,
    auto_pc: true,
    action: Action::Continue,
};

impl BlockBuilder<'_> {
    pub fn fmr(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let fpr_b = self.get(ins.fpr_b());
        self.set(ins.fpr_d(), fpr_b);

        if ins.field_rc() {
            self.update_cr1_float();
        }

        FLOAT_INFO
    }

    pub fn frsp(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let fpr_b = self.get(ins.fpr_b());

        let value = self.round_to_single(fpr_b);
        self.set(ins.fpr_d(), value);

        self.update_fprf_cmpz(value);

        if ins.field_rc() {
            self.update_cr1_float();
        }

        FLOAT_INFO
    }

    pub fn fctiwz(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let fpr_b = self.get(ins.fpr_b());
        let int32 = self.bd.ins().fcvt_to_sint_sat(ir::types::I32, fpr_b);
        let int64 = self.bd.ins().sextend(ir::types::I64, int32);
        let float = self
            .bd
            .ins()
            .bitcast(ir::types::F64, ir::MemFlags::new(), int64);

        self.set(ins.fpr_d(), float);

        self.update_fprf_cmpz(float);

        if ins.field_rc() {
            self.update_cr1_float();
        }

        FLOAT_INFO
    }

    pub fn ps_mr(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let ps_b = self.get_ps(ins.fpr_b());
        self.set_ps(ins.fpr_d(), ps_b);

        FLOAT_INFO
    }
}
