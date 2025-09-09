mod arithmetic;
mod branch;
mod compare;
mod exception;
mod logic;
mod memory;
mod others;
mod util;

use crate::block::ContextHooks;
use common::arch::{
    Reg,
    disasm::{Ins, Opcode},
};
use cranelift::{
    codegen::ir::{self, SigRef},
    frontend,
    prelude::{InstBuilder, isa::TargetIsa},
};
use easyerr::Error;
use rustc_hash::FxHashMap;
use std::{collections::hash_map::Entry, mem::offset_of};

fn reg_ty(reg: Reg) -> ir::Type {
    match reg {
        Reg::FPR(_) => ir::types::F64,
        _ => ir::types::I32,
    }
}

#[derive(Debug, Error)]
pub enum EmitError {
    #[error("illegal instruction {f0:?}")]
    Illegal(Ins),
    #[error("unimplemented instruction {f0:?}")]
    Unimplemented(Ins),
}

struct Consts {
    ptr_type: ir::Type,
    regs_ptr: ir::Value,
    ctx_ptr: ir::Value,
    ctx_hooks_ptr: ir::Value,
    ctx_hooks_sig: FxHashMap<i32, SigRef>,
}

struct RegState {
    var: frontend::Variable,
    modified: bool,
}

pub struct BlockBuilder<'ctx> {
    bd: frontend::FunctionBuilder<'ctx>,
    consts: Consts,
    regs: FxHashMap<Reg, RegState>,
    current_bb: ir::Block,

    executed: u32,
    ibat_changed: bool,
    dbat_changed: bool,
}

impl<'ctx> BlockBuilder<'ctx> {
    pub fn new(
        isa: &'ctx dyn TargetIsa,
        func: &'ctx mut ir::Function,
        ctx: &'ctx mut frontend::FunctionBuilderContext,
    ) -> Self {
        let mut builder = frontend::FunctionBuilder::new(func, ctx);
        let entry_bb = builder.create_block();
        builder.append_block_params_for_function_params(entry_bb);
        builder.switch_to_block(entry_bb);
        builder.seal_block(entry_bb);

        let ptr_type = isa.pointer_type();
        let params = builder.block_params(entry_bb);
        let ctx_ptr = params[0];
        let ctx_hooks_ptr = params[1];

        // extract regs ptr
        let signature = builder.import_signature(ContextHooks::get_registers_sig(ptr_type));
        let get_registers = builder.ins().load(
            ptr_type,
            ir::MemFlags::trusted(),
            ctx_hooks_ptr,
            offset_of!(ContextHooks, get_registers) as i32,
        );

        let inst = builder
            .ins()
            .call_indirect(signature, get_registers, &[ctx_ptr]);

        let regs_ptr = builder.inst_results(inst)[0];

        let ctx = Consts {
            ptr_type,
            regs_ptr,
            ctx_ptr,
            ctx_hooks_ptr,
            ctx_hooks_sig: FxHashMap::default(),
        };

        Self {
            bd: builder,
            consts: ctx,
            regs: FxHashMap::default(),
            current_bb: entry_bb,
            executed: 0,

            ibat_changed: false,
            dbat_changed: false,
        }
    }

    fn get(&mut self, reg: impl Into<Reg>) -> ir::Value {
        let reg = reg.into();
        let var = match self.regs.entry(reg) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let reg_ty = reg_ty(reg);
                let dumped = self.bd.ins().load(
                    reg_ty,
                    ir::MemFlags::trusted(),
                    self.consts.regs_ptr,
                    reg.offset() as i32,
                );

                let var = self.bd.declare_var(reg_ty);
                self.bd.def_var(var, dumped);
                v.insert(RegState {
                    var,
                    modified: false,
                })
            }
        }
        .var;

        self.bd.use_var(var)
    }

    fn set(&mut self, reg: impl Into<Reg>, value: ir::Value) {
        let reg = reg.into();
        let var = match self.regs.entry(reg) {
            Entry::Occupied(o) => {
                let var = o.into_mut();
                var.modified = true;

                var.var
            }
            Entry::Vacant(v) => {
                let var = self.bd.declare_var(reg_ty(reg));
                v.insert(RegState {
                    var,
                    modified: true,
                });

                var
            }
        };

        self.bd.def_var(var, value);
    }

    fn call_hook(&mut self, offset: i32) {
        let hook = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            self.consts.ctx_hooks_ptr,
            offset,
        );

        let sig = *self.consts.ctx_hooks_sig.entry(offset).or_insert_with(|| {
            self.bd
                .import_signature(ContextHooks::generic_hook_sig(self.consts.ptr_type))
        });

        self.bd
            .ins()
            .call_indirect(sig, hook, &[self.consts.ctx_ptr]);
    }

    fn prologue(&mut self) {
        self.bd.set_srcloc(ir::SourceLoc::new(u32::MAX));
        let executed = self.ir_value(self.executed);

        for (reg, var) in &self.regs {
            if !var.modified {
                continue;
            }

            let value = self.bd.use_var(var.var);
            self.bd.ins().store(
                ir::MemFlags::trusted(),
                value,
                self.consts.regs_ptr,
                reg.offset() as i32,
            );
        }

        if self.dbat_changed {
            self.call_hook(offset_of!(ContextHooks, dbat_changed) as i32);
        }

        if self.ibat_changed {
            self.call_hook(offset_of!(ContextHooks, ibat_changed) as i32);
        }

        self.bd.ins().return_(&[executed]);
    }

    pub fn emit(&mut self, ins: Ins) -> Result<(), EmitError> {
        self.bd.set_srcloc(ir::SourceLoc::new(self.executed));
        match ins.op {
            Opcode::Add => self.add(ins),
            Opcode::Addc => self.addc(ins),
            Opcode::Adde => self.adde(ins),
            Opcode::Addi => self.addi(ins),
            Opcode::Addic => self.addic(ins),
            Opcode::Addic_ => self.addic_record(ins),
            Opcode::Addis => self.addis(ins),
            Opcode::Addme => self.addme(ins),
            Opcode::Addze => self.addze(ins),
            Opcode::And => self.and(ins),
            Opcode::Andc => self.andc(ins),
            Opcode::Andi_ => self.andi_record(ins),
            Opcode::Andis_ => self.andis_record(ins),
            Opcode::B => self.b(ins),
            Opcode::Bc => self.bc(ins),
            Opcode::Bcctr => self.bcctr(ins),
            Opcode::Bclr => self.bclr(ins),
            Opcode::Cmp => self.cmp(ins),
            Opcode::Cmpi => self.cmpi(ins),
            Opcode::Cmpl => self.cmpl(ins),
            Opcode::Cmpli => self.cmpli(ins),
            Opcode::Cntlzw => self.cntlzw(ins),
            Opcode::Dcbf => self.stub(ins), // NOTE: stubbed
            Opcode::Dcbi => self.stub(ins), // NOTE: stubbed
            Opcode::Divwu => self.divwu(ins),
            Opcode::Eqv => self.eqv(ins),
            Opcode::Extsb => self.extsb(ins),
            Opcode::Extsh => self.extsh(ins),
            Opcode::Fmr => self.stub(ins),   // NOTE: stubbed
            Opcode::Icbi => self.stub(ins),  // NOTE: stubbed
            Opcode::Isync => self.stub(ins), // NOTE: stubbed
            Opcode::Lbz => self.lbz(ins),
            Opcode::Lbzx => self.lbzx(ins),
            Opcode::Lfd => self.stub(ins), // NOTE: stubbed
            Opcode::Lhz => self.lhz(ins),
            Opcode::Lhzx => self.lhzx(ins),
            Opcode::Lmw => self.lmw(ins),
            Opcode::Lwz => self.lwz(ins),
            Opcode::Lwzu => self.lwzu(ins),
            Opcode::Lwzx => self.lwzx(ins),
            Opcode::Mfcr => self.mfcr(ins),
            Opcode::Mfmsr => self.mfmsr(ins),
            Opcode::Mfspr => self.mfspr(ins),
            Opcode::Mftb => self.mftb(ins), // NOTE: stubbed
            Opcode::Mtcrf => self.mtcrf(ins),
            Opcode::Mtfsb1 => self.mtsfb1(ins),
            Opcode::Mtfsf => self.stub(ins), // NOTE: stubbed
            Opcode::Mtmsr => self.mtmsr(ins),
            Opcode::Mtspr => self.mtspr(ins),
            Opcode::Mtsr => self.mtsr(ins),
            Opcode::Mulhwu => self.mulhwu(ins),
            Opcode::Mulli => self.mulli(ins),
            Opcode::Mullw => self.mullw(ins),
            Opcode::Nand => self.nand(ins),
            Opcode::Neg => self.neg(ins),
            Opcode::Nor => self.nor(ins),
            Opcode::Or => self.or(ins),
            Opcode::Orc => self.orc(ins),
            Opcode::Ori => self.ori(ins),
            Opcode::Oris => self.oris(ins),
            Opcode::PsMr => self.stub(ins), // NOTE: stubbed
            Opcode::PsqL => self.stub(ins), // NOTE: stubbed
            Opcode::Rfi => self.rfi(ins),
            Opcode::Rlwimi => self.rlwimi(ins),
            Opcode::Rlwinm => self.rlwinm(ins),
            Opcode::Rlwnm => self.rlwnm(ins),
            Opcode::Slw => self.slw(ins),
            Opcode::Sraw => self.sraw(ins),
            Opcode::Srawi => self.srawi(ins),
            Opcode::Srw => self.srw(ins),
            Opcode::Stb => self.stb(ins),
            Opcode::Stbu => self.stbu(ins),
            Opcode::Sth => self.sth(ins),
            Opcode::Stmw => self.stmw(ins),
            Opcode::Stw => self.stw(ins),
            Opcode::Stwu => self.stwu(ins),
            Opcode::Stwx => self.stwx(ins),
            Opcode::Subf => self.subf(ins),
            Opcode::Subfc => self.subfc(ins),
            Opcode::Subfe => self.subfe(ins),
            Opcode::Subfic => self.subfic(ins),
            Opcode::Subfme => self.subfme(ins),
            Opcode::Subfze => self.subfze(ins),
            Opcode::Sync => self.stub(ins), // NOTE: stubbed
            Opcode::Xor => self.xor(ins),
            Opcode::Xori => self.xori(ins),
            Opcode::Illegal => {
                return Err(EmitError::Illegal(ins));
            }
            _ => {
                return Err(EmitError::Unimplemented(ins));
            }
        }

        let old_pc = self.get(Reg::PC);
        let new_pc = self.bd.ins().iadd_imm(old_pc, 4);
        self.set(Reg::PC, new_pc);

        self.executed += 1;
        Ok(())
    }

    pub fn finish(mut self) {
        self.prologue();
        self.bd.finalize();
    }
}
