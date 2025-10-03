use super::BlockBuilder;
use crate::builder::{Action, Info};
use common::arch::{InsExt, Reg, disasm::Ins};
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

    pub fn frsqrte(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let fpr_b = self.get(ins.fpr_b());
        let one = self.ir_value(1.0f64);

        let sqrt = self.bd.ins().sqrt(fpr_b);
        let recip = self.bd.ins().fdiv(one, sqrt);

        self.set(ins.fpr_d(), recip);

        self.update_fprf_cmpz(recip);

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

    pub fn ps_sum0(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let ps0_a = self.get(ins.fpr_a());
        let ps1_b = self.get(Reg::PS1(ins.fpr_b()));
        let ps1_c = self.get(Reg::PS1(ins.fpr_c()));

        let ps0 = self.bd.ins().fadd(ps0_a, ps1_b);
        let ps1 = ps1_c;

        self.set(ins.fpr_d(), ps0);
        self.set(Reg::PS1(ins.fpr_d()), ps1);

        self.update_fprf_cmpz(ps0);

        if ins.field_rc() {
            self.update_cr1_float();
        }

        FLOAT_INFO
    }

    pub fn ps_sum1(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let ps0_a = self.get(ins.fpr_a());
        let ps1_b = self.get(Reg::PS1(ins.fpr_b()));
        let ps0_c = self.get(ins.fpr_c());

        let ps0 = ps0_c;
        let ps1 = self.bd.ins().fadd(ps0_a, ps1_b);

        self.set(ins.fpr_d(), ps0);
        self.set(Reg::PS1(ins.fpr_d()), ps1);

        self.update_fprf_cmpz(ps0);

        if ins.field_rc() {
            self.update_cr1_float();
        }

        FLOAT_INFO
    }

    pub fn ps_merge00(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let ps0_a = self.get(ins.fpr_a());
        let ps0_b = self.get(ins.fpr_b());

        self.set(ins.fpr_d(), ps0_a);
        self.set(Reg::PS1(ins.fpr_d()), ps0_b);

        if ins.field_rc() {
            self.update_cr1_float();
        }

        FLOAT_INFO
    }

    pub fn ps_merge01(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let ps0_a = self.get(ins.fpr_a());
        let ps1_b = self.get(Reg::PS1(ins.fpr_b()));

        self.set(ins.fpr_d(), ps0_a);
        self.set(Reg::PS1(ins.fpr_d()), ps1_b);

        if ins.field_rc() {
            self.update_cr1_float();
        }

        FLOAT_INFO
    }

    pub fn ps_merge10(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let ps1_a = self.get(Reg::PS1(ins.fpr_a()));
        let ps0_b = self.get(ins.fpr_b());

        self.set(ins.fpr_d(), ps1_a);
        self.set(Reg::PS1(ins.fpr_d()), ps0_b);

        if ins.field_rc() {
            self.update_cr1_float();
        }

        FLOAT_INFO
    }

    pub fn ps_merge11(&mut self, ins: Ins) -> Info {
        self.check_floats();

        let ps1_a = self.get(Reg::PS1(ins.fpr_a()));
        let ps1_b = self.get(Reg::PS1(ins.fpr_b()));

        self.set(ins.fpr_d(), ps1_a);
        self.set(Reg::PS1(ins.fpr_d()), ps1_b);

        if ins.field_rc() {
            self.update_cr1_float();
        }

        FLOAT_INFO
    }
}
