//! # Hemisphere: a Nintendo GameCube emulator
//! This is the main crate of the hemisphere emulator (you could call it the core of the emulator).
//! The system state is defined in [`system`], with [`jit`] being glue-code to [`ppcjit`] and
//! [`runner`] being a way to setup emulation and drive it forward.

#![feature(cold_path)]

pub mod jit;
pub mod runner;
pub mod system;

use crate::{
    jit::{CTX_HOOKS, Context, JIT},
    system::System,
};
use common::arch::disasm::{Extensions, Ins, ParsedIns};
use ppcjit::{Sequence, SequenceStatus, block::Executed};
use tracing::{trace, trace_span};

pub use common::{self, Address, Primitive, arch};
pub use dol;

/// Emulator configuration.
pub struct Config {
    /// Maximum number of instructions per JIT block.
    pub instr_per_block: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            instr_per_block: 512,
        }
    }
}

/// Represents limits for execution.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// A hard-limit on how many instructions can be executed. This means that the number of
    /// executed instructions will always be less than or equal to this value.
    pub instructions: u32,
    /// A soft-limit on how many cycles can be executed. This means that the number of executed
    /// instructions might be less than this value or, at most, slightly above it.
    pub cycles: u32,
}

impl Limits {
    pub fn instructions(value: u32) -> Self {
        Self {
            instructions: value,
            cycles: u32::MAX,
        }
    }

    pub fn cycles(value: u32) -> Self {
        Self {
            instructions: u32::MAX,
            cycles: value,
        }
    }
}

/// The Hemisphere emulator.
pub struct Hemisphere {
    pub config: Config,
    pub system: System,
    pub jit: JIT,
}

impl Hemisphere {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            system: System::new(),
            jit: JIT::new(),
        }
    }

    /// Compiles a sequence of at most `limit` instructions starting at `addr` into a JIT block.
    fn compile(&mut self, addr: Address, limit: u32) -> ppcjit::Block {
        let _span = trace_span!("compiling new block", addr = ?self.system.cpu.pc).entered();

        let mut seq = Sequence::new();
        let mut current = addr;

        loop {
            if seq.len() >= limit as usize {
                break;
            }

            let physical = self.system.translate_instr_addr(current);
            let ins = Ins::new(self.system.bus.read(physical), Extensions::gekko_broadway());

            let mut parsed = ParsedIns::new();
            ins.parse_basic(&mut parsed);

            match seq.push(ins) {
                Ok(SequenceStatus::Open) => {
                    if current == u32::MAX {
                        break;
                    } else {
                        current += 4
                    }
                }
                _ => break,
            }
        }

        trace!(instructions = seq.len(), "block sequence built");
        self.jit.compiler.compile(seq).unwrap()
    }

    #[inline(always)]
    fn exec_inner(&mut self, instr_limit: u32) -> Executed {
        let block = self
            .jit
            .blocks
            .mapping
            .get(self.system.cpu.pc)
            .and_then(|id| self.jit.blocks.storage.get(id))
            .filter(|b| b.sequence().len() <= instr_limit as usize);

        let compiled: ppcjit::Block;
        let block = match block {
            Some(block) => block,
            None => {
                std::hint::cold_path();

                compiled = self.compile(self.system.cpu.pc, instr_limit);
                &compiled
            }
        };

        let mut ctx = Context {
            system: &mut self.system,
            mapping: &mut self.jit.blocks.mapping,
        };

        block.run(&mut ctx as *mut _ as *mut _, &CTX_HOOKS)
    }

    fn exec(&mut self, limits: Limits) -> Executed {
        let block = self
            .jit
            .blocks
            .mapping
            .get(self.system.cpu.pc)
            .and_then(|id| self.jit.blocks.storage.get(id));

        if block.is_none() {
            let block = self.compile(self.system.cpu.pc, self.config.instr_per_block);
            self.jit.blocks.insert(self.system.cpu.pc, block);
        }

        self.exec_inner(limits.instructions)
    }

    pub fn step(&mut self) -> Executed {
        self.exec(Limits {
            instructions: 1,
            cycles: u32::MAX,
        })
    }

    /// Runs the emulator respecting the given `limits`.
    pub fn run(&mut self, limits: Limits) -> Executed {
        let mut executed = Executed::default();
        let mut remaining_instr = limits.instructions;
        let mut remaining_cycles = limits.cycles;

        while remaining_cycles > 0 && remaining_instr > 0 {
            let until_next_event = self.system.scheduler.until_next().unwrap_or(u64::MAX);
            let cycles_to_run = until_next_event
                .min(remaining_cycles as u64)
                .min(u32::MAX as u64) as u32;

            // tracing::debug!("executing at {}", self.system.cpu.pc);
            let e = self.exec(Limits {
                cycles: cycles_to_run,
                instructions: remaining_instr,
            });

            executed.instructions += e.instructions;
            executed.cycles += e.cycles;

            remaining_instr = remaining_instr.saturating_sub(e.instructions);
            remaining_cycles = remaining_cycles.saturating_sub(e.cycles);

            // process all events
            self.system.scheduler.advance(e.cycles as u64);
            while let Some(event) = self.system.scheduler.pop() {
                self.system.process(event);
            }
        }

        executed
    }
}
