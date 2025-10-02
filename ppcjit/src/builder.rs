mod arithmetic;
mod branch;
mod compare;
mod exception;
mod floating;
mod logic;
mod memory;
mod others;
mod util;

use crate::{Sequence, block::Hooks, builder::util::IntoIrValue};
use common::arch::{
    FPR, Reg, SPR,
    disasm::{Ins, Opcode},
};
use cranelift::{
    codegen::ir::{self, SigRef},
    frontend,
    prelude::{InstBuilder, isa::TargetIsa},
};
use easyerr::Error;
use rustc_hash::FxHashMap;
use std::mem::offset_of;

// NOTE: make sure to keep this up to date if anything else is not just 32 bits
fn reg_ir_ty(reg: Reg) -> ir::Type {
    match reg {
        Reg::FPR(_) | Reg::PS1(_) => ir::types::F64,
        _ => ir::types::I32,
    }
}

fn is_cacheable(reg: Reg) -> bool {
    match reg {
        Reg::SPR(spr) => match spr {
            SPR::DEC | SPR::TBL | SPR::TBU => false,
            spr if spr.is_bat() => false,
            _ => true,
        },
        _ => true,
    }
}

#[derive(Debug, Error)]
pub enum BuilderError {
    #[error("illegal instruction {f0:?}")]
    Illegal(Ins),
    #[error("unimplemented instruction {f0:?}")]
    Unimplemented(Ins),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Continue,
    FlushAndPrologue,
    Prologue,
}

pub(crate) struct Info {
    cycles: u8,
    auto_pc: bool,
    action: Action,
}

/// Constants used through block building.
struct Consts {
    ptr_type: ir::Type,

    regs_ptr: ir::Value,
    ctx_ptr: ir::Value,
    hooks_ptr: ir::Value,

    hooks_sig: FxHashMap<i32, SigRef>,
    raise_exception_sig: Option<SigRef>,
}

/// A cached value.
struct CachedValue {
    value: ir::Value,
    modified: bool,
}

/// Structure to build JIT blocks.
pub struct BlockBuilder<'ctx> {
    bd: frontend::FunctionBuilder<'ctx>,
    cache: FxHashMap<Reg, CachedValue>,
    ps_cache: FxHashMap<FPR, CachedValue>,
    consts: Consts,
    current_bb: ir::Block,

    cycles: u32,
    executed: u32,
    ibat_changed: bool,
    dbat_changed: bool,
    floats_checked: bool,
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
        let hooks_ptr = params[1];

        // extract regs ptr
        let signature = builder.import_signature(Hooks::get_registers_sig(ptr_type));
        let get_registers = builder.ins().load(
            ptr_type,
            ir::MemFlags::trusted(),
            hooks_ptr,
            offset_of!(Hooks, get_registers) as i32,
        );

        let inst = builder
            .ins()
            .call_indirect(signature, get_registers, &[ctx_ptr]);

        let regs_ptr = builder.inst_results(inst)[0];

        let consts = Consts {
            ptr_type,
            regs_ptr,
            ctx_ptr,
            hooks_ptr,
            hooks_sig: FxHashMap::default(),
            raise_exception_sig: None,
        };

        Self {
            bd: builder,
            cache: FxHashMap::default(),
            ps_cache: FxHashMap::default(),
            consts,
            current_bb: entry_bb,

            cycles: 0,
            executed: 0,
            ibat_changed: false,
            dbat_changed: false,
            floats_checked: false,
        }
    }

    fn switch_to_bb(&mut self, bb: ir::Block) {
        self.bd.switch_to_block(bb);
        self.bd.set_srcloc(ir::SourceLoc::new(self.executed));
        self.current_bb = bb;
    }

    fn load_reg(&mut self, reg: Reg) -> ir::Value {
        let reg_ty = reg_ir_ty(reg);
        self.bd.ins().load(
            reg_ty,
            ir::MemFlags::trusted(),
            self.consts.regs_ptr,
            reg.offset() as i32,
        )
    }

    fn store_reg(&mut self, reg: Reg, value: ir::Value) {
        self.bd.ins().store(
            ir::MemFlags::trusted(),
            value,
            self.consts.regs_ptr,
            reg.offset() as i32,
        );
    }

    /// Gets the current value of the given register.
    fn get(&mut self, reg: impl Into<Reg>) -> ir::Value {
        let reg = reg.into();

        if let Reg::FPR(fpr) | Reg::PS1(fpr) = reg {
            self.flush_ps(fpr);
        }

        if let Some(reg) = self.cache.get(&reg) {
            return reg.value;
        }

        let dumped = self.load_reg(reg);
        if is_cacheable(reg) {
            self.cache.insert(
                reg,
                CachedValue {
                    value: dumped,
                    modified: false,
                },
            );
        }

        dumped
    }

    /// Sets the value of the given register.
    fn set(&mut self, reg: impl Into<Reg>, value: impl IntoIrValue) {
        let reg = reg.into();
        let value = self.ir_value(value);

        if let Reg::FPR(fpr) | Reg::PS1(fpr) = reg {
            self.invalidate_ps(fpr);
        }

        if let Some(reg) = self.cache.get_mut(&reg) {
            reg.value = value;
            reg.modified = true;
            return;
        }

        if is_cacheable(reg) {
            self.cache.insert(
                reg,
                CachedValue {
                    value,
                    modified: true,
                },
            );
        } else {
            self.store_reg(reg, value);
        }
    }

    fn get_ps(&mut self, fpr: FPR) -> ir::Value {
        if let Some(val) = self.ps_cache.get(&fpr) {
            return val.value;
        }

        let ps0 = self.get(fpr);
        let ps1 = self.get(Reg::PS1(fpr));

        let paired = self.bd.ins().splat(ir::types::F64X2, ps0);
        let paired = self.bd.ins().insertlane(paired, ps1, 1);

        self.ps_cache.insert(
            fpr,
            CachedValue {
                value: paired,
                modified: false,
            },
        );

        paired
    }

    fn set_ps(&mut self, fpr: FPR, value: ir::Value) {
        if let Some(val) = self.ps_cache.get_mut(&fpr) {
            val.modified = true;
            val.value = value;
            return;
        }

        self.ps_cache.insert(
            fpr,
            CachedValue {
                value,
                modified: true,
            },
        );
    }

    fn flush_ps(&mut self, fpr: FPR) {
        let Some(val) = self.ps_cache.get_mut(&fpr) else {
            return;
        };

        if !val.modified {
            return;
        }

        val.modified = false;

        let ps0 = self.bd.ins().extractlane(val.value, 0);
        let ps1 = self.bd.ins().extractlane(val.value, 1);

        self.cache.insert(
            Reg::FPR(fpr),
            CachedValue {
                value: ps0,
                modified: true,
            },
        );
        self.cache.insert(
            Reg::PS1(fpr),
            CachedValue {
                value: ps1,
                modified: true,
            },
        );
    }

    fn invalidate_ps(&mut self, fpr: FPR) {
        self.flush_ps(fpr);
        self.ps_cache.remove(&fpr);
    }

    /// Calls a generic context hook.
    fn call_generic_hook(&mut self, offset: i32) {
        let hook = self.bd.ins().load(
            self.consts.ptr_type,
            ir::MemFlags::trusted(),
            self.consts.hooks_ptr,
            offset,
        );

        let sig = *self.consts.hooks_sig.entry(offset).or_insert_with(|| {
            self.bd
                .import_signature(Hooks::generic_hook_sig(self.consts.ptr_type))
        });

        self.bd
            .ins()
            .call_indirect(sig, hook, &[self.consts.ctx_ptr]);
    }

    /// Flushes the register cache to the registers struct.
    fn flush(&mut self) {
        for (&fpr, val) in &self.ps_cache {
            if !val.modified {
                continue;
            }

            self.bd.ins().store(
                ir::MemFlags::trusted(),
                val.value,
                self.consts.regs_ptr,
                fpr.offset() as i32,
            );
        }

        for (reg, val) in &self.cache {
            // check whether this reg is a FPR/PS1 that has already been flushed
            if let Reg::FPR(fpr) | Reg::PS1(fpr) = reg
                && let Some(val) = self.ps_cache.get(fpr)
                && val.modified
            {
                continue;
            }

            if !val.modified {
                continue;
            }

            self.bd.ins().store(
                ir::MemFlags::trusted(),
                val.value,
                self.consts.regs_ptr,
                reg.offset() as i32,
            );
        }
    }

    /// Emits the prologue:
    /// - Call BAT hooks if they were changed
    /// - Returns
    fn prologue(&mut self) {
        let instructions = self.ir_value(self.executed);
        let instructions = self.bd.ins().uextend(ir::types::I64, instructions);
        let cycles = self.ir_value(self.cycles);
        let cycles = self.bd.ins().uextend(ir::types::I64, cycles);
        let cycles = self.bd.ins().ishl_imm(cycles, 32);
        let merged = self.bd.ins().bor(instructions, cycles);

        if self.dbat_changed {
            self.call_generic_hook(offset_of!(Hooks, dbat_changed) as i32);
        }

        if self.ibat_changed {
            self.call_generic_hook(offset_of!(Hooks, ibat_changed) as i32);
        }

        self.bd.ins().return_(&[merged]);
        self.bd.set_srcloc(ir::SourceLoc::new(self.executed));
    }

    fn prologue_with(&mut self, info: Info) {
        self.executed += 1;
        self.cycles += info.cycles as u32;

        self.prologue();

        self.executed -= 1;
        self.cycles -= info.cycles as u32;
    }

    /// Emits the given instruction into the block.
    fn emit(&mut self, ins: Ins) -> Result<Action, BuilderError> {
        self.bd.set_srcloc(ir::SourceLoc::new(self.executed));
        let info: Info = match ins.op {
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
            Opcode::Crxor => self.crxor(ins),
            Opcode::Dcbf => self.nop(Action::Continue),
            Opcode::Dcbi => self.nop(Action::Continue),
            Opcode::Divw => self.divw(ins),
            Opcode::Divwu => self.divwu(ins),
            Opcode::Eqv => self.eqv(ins),
            Opcode::Extsb => self.extsb(ins),
            Opcode::Extsh => self.extsh(ins),
            Opcode::Fmr => self.fmr(ins),
            Opcode::Icbi => self.nop(Action::Continue),
            Opcode::Isync => self.nop(Action::FlushAndPrologue),
            Opcode::Lbz => self.lbz(ins),
            Opcode::Lbzu => self.lbzu(ins),
            Opcode::Lbzux => self.lbzux(ins),
            Opcode::Lbzx => self.lbzx(ins),
            Opcode::Lfd => self.lfd(ins),
            Opcode::Lfs => self.lfs(ins),
            Opcode::Lha => self.lha(ins),
            Opcode::Lhau => self.lhau(ins),
            Opcode::Lhaux => self.lhaux(ins),
            Opcode::Lhax => self.lhax(ins),
            Opcode::Lhz => self.lhz(ins),
            Opcode::Lhzu => self.lhzu(ins),
            Opcode::Lhzux => self.lhzux(ins),
            Opcode::Lhzx => self.lhzx(ins),
            Opcode::Lmw => self.lmw(ins),
            Opcode::Lwz => self.lwz(ins),
            Opcode::Lwzu => self.lwzu(ins),
            Opcode::Lwzux => self.lwzux(ins),
            Opcode::Lwzx => self.lwzx(ins),
            Opcode::Mcrf => self.mcrf(ins),
            Opcode::Mfcr => self.mfcr(ins),
            Opcode::Mfmsr => self.mfmsr(ins),
            Opcode::Mfspr => self.mfspr(ins),
            Opcode::Mftb => self.mftb(ins),
            Opcode::Mtcrf => self.mtcrf(ins),
            Opcode::Mtfsb1 => self.mtfsb1(ins),
            Opcode::Mtfsf => self.mtfsf(ins),
            Opcode::Mtmsr => self.mtmsr(ins),
            Opcode::Mtspr => self.mtspr(ins),
            Opcode::Mtsr => self.mtsr(ins),
            Opcode::Mulhw => self.mulhw(ins),
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
            Opcode::PsMr => self.ps_mr(ins),
            Opcode::PsqL => self.psq_l(ins),
            Opcode::Rfi => self.rfi(ins),
            Opcode::Rlwimi => self.rlwimi(ins),
            Opcode::Rlwinm => self.rlwinm(ins),
            Opcode::Rlwnm => self.rlwnm(ins),
            Opcode::Sc => self.sc(ins),
            Opcode::Slw => self.slw(ins),
            Opcode::Sraw => self.sraw(ins),
            Opcode::Srawi => self.srawi(ins),
            Opcode::Srw => self.srw(ins),
            Opcode::Stb => self.stb(ins),
            Opcode::Stbu => self.stbu(ins),
            Opcode::Stbux => self.stbux(ins),
            Opcode::Stbx => self.stbx(ins),
            Opcode::Stfd => self.stfd(ins),
            Opcode::Sth => self.sth(ins),
            Opcode::Sthu => self.sthu(ins),
            Opcode::Sthux => self.sthux(ins),
            Opcode::Sthx => self.sthx(ins),
            Opcode::Stmw => self.stmw(ins),
            Opcode::Stw => self.stw(ins),
            Opcode::Stwu => self.stwu(ins),
            Opcode::Stwux => self.stwux(ins),
            Opcode::Stwx => self.stwx(ins),
            Opcode::Subf => self.subf(ins),
            Opcode::Subfc => self.subfc(ins),
            Opcode::Subfe => self.subfe(ins),
            Opcode::Subfic => self.subfic(ins),
            Opcode::Subfme => self.subfme(ins),
            Opcode::Subfze => self.subfze(ins),
            Opcode::Sync => self.nop(Action::FlushAndPrologue),
            Opcode::Xor => self.xor(ins),
            Opcode::Xori => self.xori(ins),
            Opcode::PsqSt => self.psq_st(ins),
            Opcode::Illegal => {
                return Err(BuilderError::Illegal(ins));
            }
            _ => {
                return Err(BuilderError::Unimplemented(ins));
            }
        };

        self.executed += 1;
        self.cycles += info.cycles as u32;

        if info.auto_pc {
            let old_pc = self.get(Reg::PC);
            let new_pc = self.bd.ins().iadd_imm(old_pc, 4);
            self.set(Reg::PC, new_pc);
        }

        Ok(info.action)
    }

    pub fn build(
        mut self,
        mut instructions: impl Iterator<Item = Ins>,
    ) -> Result<(Sequence, u32), BuilderError> {
        let mut sequence = Sequence::default();
        loop {
            let Some(ins) = instructions.next() else {
                self.bd.set_srcloc(ir::SourceLoc::new(u32::MAX));
                self.flush();
                self.prologue();
                self.bd.finalize();
                break;
            };

            sequence.0.push(ins);

            match self.emit(ins)? {
                Action::Continue => (),
                Action::FlushAndPrologue => {
                    self.bd.set_srcloc(ir::SourceLoc::new(u32::MAX));
                    self.flush();
                    self.prologue();
                    self.bd.finalize();
                    break;
                }
                Action::Prologue => {
                    self.bd.set_srcloc(ir::SourceLoc::new(u32::MAX));
                    self.prologue();
                    self.bd.finalize();
                    break;
                }
            }
        }

        Ok((sequence, self.cycles))
    }
}
