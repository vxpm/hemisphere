mod arithmetic;
mod branch;
mod compare;
mod exception;
mod logic;
mod memory;
mod others;
mod util;

use crate::{Sequence, block::Hooks, builder::util::IntoIrValue};
use common::arch::{
    Reg, SPR,
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

// NOTE: make sure to keep this up to date if anything else is not just 32 bits
fn reg_ir_ty(reg: Reg) -> ir::Type {
    match reg {
        Reg::FPR(_) => ir::types::F64,
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

/// A cached register.
struct CachedReg {
    var: frontend::Variable,
    modified: bool,
}

/// Structure to build JIT blocks.
pub struct BlockBuilder<'ctx> {
    bd: frontend::FunctionBuilder<'ctx>,
    cache: FxHashMap<Reg, CachedReg>,
    consts: Consts,
    current_bb: ir::Block,

    cycles: u32,
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
            consts,
            current_bb: entry_bb,

            cycles: 0,
            executed: 0,
            ibat_changed: false,
            dbat_changed: false,
        }
    }

    fn switch_to_bb(&mut self, bb: ir::Block) {
        self.bd.switch_to_block(bb);
        self.bd.set_srcloc(ir::SourceLoc::new(self.executed));
        self.current_bb = bb;
    }

    /// Gets the current value of the given register.
    fn get(&mut self, reg: impl Into<Reg>) -> ir::Value {
        let reg = reg.into();

        if is_cacheable(reg) {
            let var = match self.cache.entry(reg) {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => {
                    let reg_ty = reg_ir_ty(reg);
                    let dumped = self.bd.ins().load(
                        reg_ty,
                        ir::MemFlags::trusted(),
                        self.consts.regs_ptr,
                        reg.offset() as i32,
                    );

                    let var = self.bd.declare_var(reg_ty);
                    self.bd.def_var(var, dumped);
                    v.insert(CachedReg {
                        var,
                        modified: false,
                    })
                }
            }
            .var;

            self.bd.use_var(var)
        } else {
            let reg_ty = reg_ir_ty(reg);
            self.bd.ins().load(
                reg_ty,
                ir::MemFlags::trusted(),
                self.consts.regs_ptr,
                reg.offset() as i32,
            )
        }
    }

    /// Sets the value of the given register.
    fn set(&mut self, reg: impl Into<Reg>, value: impl IntoIrValue) {
        let reg = reg.into();

        if is_cacheable(reg) {
            let var = match self.cache.entry(reg) {
                Entry::Occupied(o) => {
                    let var = o.into_mut();
                    var.modified = true;

                    var.var
                }
                Entry::Vacant(v) => {
                    let var = self.bd.declare_var(reg_ir_ty(reg));
                    v.insert(CachedReg {
                        var,
                        modified: true,
                    });

                    var
                }
            };

            let value = self.ir_value(value);
            self.bd.def_var(var, value);
        } else {
            let value = self.ir_value(value);
            self.bd.ins().store(
                ir::MemFlags::trusted(),
                value,
                self.consts.regs_ptr,
                reg.offset() as i32,
            );
        }
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
        for (reg, var) in &self.cache {
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
    }

    /// Emits the prologue:
    /// - Call BAT hooks if they were changed
    /// - Returns
    fn prologue(&mut self) {
        self.bd.set_srcloc(ir::SourceLoc::new(u32::MAX));
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
            Opcode::Dcbf => self.stub(ins), // NOTE: stubbed
            Opcode::Dcbi => self.stub(ins), // NOTE: stubbed
            Opcode::Divw => self.divw(ins),
            Opcode::Divwu => self.divwu(ins),
            Opcode::Eqv => self.eqv(ins),
            Opcode::Extsb => self.extsb(ins),
            Opcode::Extsh => self.extsh(ins),
            Opcode::Fmr => self.stub(ins),   // NOTE: stubbed
            Opcode::Icbi => self.stub(ins),  // NOTE: stubbed
            Opcode::Isync => self.stub(ins), // NOTE: stubbed
            Opcode::Lbz => self.lbz(ins),
            Opcode::Lbzu => self.lbzu(ins),
            Opcode::Lbzux => self.lbzux(ins),
            Opcode::Lbzx => self.lbzx(ins),
            Opcode::Lfd => self.stub(ins), // NOTE: stubbed
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
            Opcode::Mfcr => self.mfcr(ins),
            Opcode::Mfmsr => self.mfmsr(ins),
            Opcode::Mfspr => self.mfspr(ins),
            Opcode::Mftb => self.mftb(ins), // NOTE: stubbed
            Opcode::Mtcrf => self.mtcrf(ins),
            Opcode::Mtfsb1 => self.stub(ins),
            Opcode::Mtfsf => self.stub(ins), // NOTE: stubbed
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
            Opcode::PsMr => self.stub(ins), // NOTE: stubbed
            Opcode::PsqL => self.stub(ins), // NOTE: stubbed
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
            Opcode::Sync => self.stub(ins), // NOTE: stubbed
            Opcode::Xor => self.xor(ins),
            Opcode::Xori => self.xori(ins),
            Opcode::Crxor => self.crxor(ins),
            Opcode::Stfd => self.stub(ins), // NOTE: stubbed
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
                self.flush();
                self.prologue();
                self.bd.finalize();
                break;
            };

            sequence.0.push(ins);

            match self.emit(ins)? {
                Action::Continue => (),
                Action::FlushAndPrologue => {
                    self.flush();
                    self.prologue();
                    self.bd.finalize();
                    break;
                }
                Action::Prologue => {
                    self.prologue();
                    self.bd.finalize();
                    break;
                }
            }
        }

        Ok((sequence, self.cycles))
    }
}
