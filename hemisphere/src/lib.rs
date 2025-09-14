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
use ppcjit::{Sequence, SequenceStatus};
use tracing::{trace, trace_span};

pub use common::{self, Address, Primitive, arch};
pub use dol;

/// Emulator configuration.
pub struct Config {
    /// Maximum number of instructions per JIT block.
    pub instr_per_block: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            instr_per_block: 256,
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
    fn compile(&mut self, addr: Address, limit: u16) -> ppcjit::Block {
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

    /// Executes a single block and returns how many cycles were executed. The block must execute
    /// at most `limit` instructions.
    ///
    /// If there's no block in storage which meets the requirements, a block will be compiled for
    /// execution then immediately get discarded.
    pub fn exec_with_limit(&mut self, limit: u16) -> u32 {
        let block = self
            .jit
            .blocks
            .mapping
            .get(self.system.cpu.pc)
            .and_then(|id| self.jit.blocks.storage.get(id))
            .filter(|b| b.sequence().len() <= limit as usize);

        let compiled: ppcjit::Block;
        let block = match block {
            Some(block) => block,
            None => {
                std::hint::cold_path();

                compiled = self.compile(self.system.cpu.pc, limit);
                &compiled
            }
        };

        let mut ctx = Context {
            system: &mut self.system,
            mapping: &mut self.jit.blocks.mapping,
        };

        block.run(&mut ctx as *mut _ as *mut _, &CTX_HOOKS)
    }

    /// Executes a single block and returns how many cycles were executed. The block must execute
    /// at most `limit` instructions.
    ///
    /// If there's no block in storage which meets the requirements, a block will be compiled with
    /// normal limits and get cached. If the compiled block _still_ doesn't meet the requirements,
    /// a block will be compiled for execution then immediately get discarded.
    fn exec_with_limit_and_try_cache(&mut self, limit: u16) -> u32 {
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

        self.exec_with_limit(limit)
    }

    /// Executes a single block and returns how many cycles were executed.
    pub fn exec(&mut self) -> u32 {
        // tracing::debug!("exec at {}", self.system.cpu.pc);

        let block = self
            .jit
            .blocks
            .mapping
            .get(self.system.cpu.pc)
            .and_then(|id| self.jit.blocks.storage.get(id));

        let block = match block {
            Some(block) => block,
            None => {
                std::hint::cold_path();

                let block = self.compile(self.system.cpu.pc, self.config.instr_per_block);
                let id = self.jit.blocks.insert(self.system.cpu.pc, block);

                self.jit.blocks.storage.get(id).unwrap()
            }
        };

        // tracing::debug!(
        //     "====> block seq:\n{}\n",
        //     block.sequence(),
        //     // block.clir(),
        // );

        let mut ctx = Context {
            system: &mut self.system,
            mapping: &mut self.jit.blocks.mapping,
        };

        block.run(&mut ctx as *mut _ as *mut _, &CTX_HOOKS)
    }
}
