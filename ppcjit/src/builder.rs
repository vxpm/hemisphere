mod arithmetic;
mod branch;
mod compare;
mod exception;
mod memory;
mod others;

use crate::block::ContextHooks;
use cranelift::{
    codegen::ir::{self, SigRef, condcodes::IntCC},
    frontend,
    prelude::{
        InstBuilder,
        isa::{CallConv, TargetIsa},
    },
};
use easyerr::Error;
use hemicore::arch::{
    Reg, SPR,
    powerpc::{Ins, Opcode},
};
use rustc_hash::FxHashMap;
use std::{collections::hash_map::Entry, mem::offset_of};

fn sig_get_registers(ptr_type: ir::Type) -> ir::Signature {
    ir::Signature {
        params: vec![
            ir::AbiParam::new(ptr_type), // ctx
        ],
        returns: vec![ir::AbiParam::new(ptr_type)],
        call_conv: CallConv::SystemV,
    }
}

fn sig_generic_hook(ptr_type: ir::Type) -> ir::Signature {
    ir::Signature {
        params: vec![
            ir::AbiParam::new(ptr_type), // ctx
        ],
        returns: vec![],
        call_conv: CallConv::SystemV,
    }
}

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
        let signature = builder.import_signature(sig_get_registers(ptr_type));
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

    fn update_xer_ov(&mut self, overflowed: ir::Value) {
        let xer = self.get(SPR::XER);
        let overflowed = self.bd.ins().uextend(ir::types::I32, overflowed);

        let ov = self.bd.ins().ishl_imm(overflowed, 30);
        let so = self.bd.ins().ishl_imm(overflowed, 31);
        let value = self.bd.ins().bor(ov, so);

        let mask = self.bd.ins().iconst(ir::types::I32, !(0b1 << 30));
        let masked = self.bd.ins().band(xer, mask);
        let updated = self.bd.ins().bor(masked, value);

        self.set(SPR::XER, updated);
    }

    fn update_xer_ca(&mut self, carry: ir::Value) {
        let xer = self.get(SPR::XER);
        let carry = self.bd.ins().uextend(ir::types::I32, carry);
        let value = self.bd.ins().ishl_imm(carry, 29);

        let mask = self.bd.ins().iconst(ir::types::I32, !(0b1 << 29));
        let masked = self.bd.ins().band(xer, mask);
        let updated = self.bd.ins().bor(masked, value);

        self.set(SPR::XER, updated);
    }

    /// All ir values must be booleans (i.e. I8).
    fn update_cr(&mut self, index: u8, lt: ir::Value, gt: ir::Value, eq: ir::Value, ov: ir::Value) {
        let cr = self.get(Reg::CR);

        let lt = self.bd.ins().uextend(ir::types::I32, lt);
        let gt = self.bd.ins().uextend(ir::types::I32, gt);
        let eq = self.bd.ins().uextend(ir::types::I32, eq);
        let ov = self.bd.ins().uextend(ir::types::I32, ov);

        let base = (4 * (7 - index)) as u64 as i64;
        let lt = self.bd.ins().ishl_imm(lt, base + 3);
        let gt = self.bd.ins().ishl_imm(gt, base + 2);
        let eq = self.bd.ins().ishl_imm(eq, base + 1);
        let ov = self.bd.ins().ishl_imm(ov, base + 0);

        let value = self.bd.ins().bor(lt, gt);
        let value = self.bd.ins().bor(value, eq);
        let value = self.bd.ins().bor(value, ov);

        let mask = self.bd.ins().iconst(ir::types::I32, 0b1111 << base);
        let updated = self.bd.ins().bitselect(mask, value, cr);

        self.set(Reg::CR, updated);
    }

    fn update_cr0_implicit(&mut self, value: ir::Value, overflowed: ir::Value) {
        let lt = self.bd.ins().icmp_imm(IntCC::SignedLessThan, value, 0);
        let gt = self.bd.ins().icmp_imm(IntCC::SignedGreaterThan, value, 0);
        let eq = self.bd.ins().icmp_imm(IntCC::Equal, value, 0);

        self.update_cr(0, lt, gt, eq, overflowed);
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
                .import_signature(sig_generic_hook(self.consts.ptr_type))
        });

        self.bd
            .ins()
            .call_indirect(sig, hook, &[self.consts.ctx_ptr]);
    }

    fn prologue(&mut self) {
        let executed = self
            .bd
            .ins()
            .iconst(ir::types::I32, self.executed as u64 as i64);

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
            Opcode::Addi => self.addi(ins),
            Opcode::Addic_ => self.addic_record(ins),
            Opcode::Addis => self.addis(ins),
            Opcode::And => self.and(ins),
            Opcode::Andi_ => self.andi_record(ins),
            Opcode::B => self.branch(ins),
            Opcode::Bc => self.branch_cond(ins),
            Opcode::Bclr => self.branch_cond_lr(ins),
            Opcode::Cmp => self.cmp(ins),
            Opcode::Cmpi => self.cmpi(ins),
            Opcode::Cmpl => self.cmpl(ins),
            Opcode::Cmpli => self.cmpli(ins),
            Opcode::Dcbf => self.stub(ins), // NOTE: stubbed
            Opcode::Divwu => self.divwu(ins),
            Opcode::Fmr => self.stub(ins),   // NOTE: stubbed
            Opcode::Isync => self.stub(ins), // NOTE: stubbed
            Opcode::Lbz => self.lbz(ins),
            Opcode::Lbzx => self.lbzx(ins),
            Opcode::Lfd => self.stub(ins), // NOTE: stubbed
            Opcode::Lmw => self.lmw(ins),
            Opcode::Lwz => self.lwz(ins),
            Opcode::Lwzu => self.lwzu(ins),
            Opcode::Lwzx => self.lwzx(ins),
            Opcode::Mfmsr => self.mfmsr(ins),
            Opcode::Mfspr => self.mfspr(ins),
            Opcode::Mtfsb1 => self.mtsfb1(ins),
            Opcode::Mtfsf => self.stub(ins), // NOTE: stubbed
            Opcode::Mtmsr => self.mtmsr(ins),
            Opcode::Mtspr => self.mtspr(ins),
            Opcode::Mtsr => self.mtsr(ins),
            Opcode::Mullw => self.mullw(ins),
            Opcode::Or => self.or(ins),
            Opcode::Ori => self.ori(ins),
            Opcode::Oris => self.oris(ins),
            Opcode::PsMr => self.stub(ins), // NOTE: stubbed
            Opcode::PsqL => self.stub(ins), // NOTE: stubbed
            Opcode::Rfi => self.rfi(ins),
            Opcode::Rlwinm => self.rlwinm(ins),
            Opcode::Stb => self.stb(ins),
            Opcode::Stbu => self.stbu(ins),
            Opcode::Sth => self.sth(ins),
            Opcode::Stmw => self.stmw(ins),
            Opcode::Stw => self.stw(ins),
            Opcode::Stwu => self.stwu(ins),
            Opcode::Stwx => self.stwx(ins),
            Opcode::Subf => self.subf(ins),
            Opcode::Sync => self.stub(ins), // NOTE: stubbed
            Opcode::Icbi => self.stub(ins), // NOTE: stubbed
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
