//! # Hemisphere: a Nintendo GameCube emulator
//! This is the main crate of the hemisphere emulator (you could call it the core of the emulator).
//! The system state is defined in [`system`], with [`jit`] being glue-code to [`ppcjit`] and
//! [`runner`] being a way to setup emulation and drive it forward.

#![feature(cold_path)]

pub mod jit;
pub mod panic;
pub mod runner;
pub mod system;

use crate::{
    jit::{CTX_HOOKS, Context, JIT},
    system::System,
};
use common::arch::disasm::{Extensions, Ins};
use ppcjit::block::Executed;
use tracing::{debug, trace, trace_span};

pub use common::{self, Address, Primitive, arch};
pub use dol;

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

/// Emulator configuration.
pub struct Config {
    pub system: system::Config,
    pub jit: jit::Config,
}

/// The Hemisphere emulator.
pub struct Hemisphere {
    pub system: System,
    pub jit: JIT,
}

impl Hemisphere {
    pub fn new(config: Config) -> Self {
        Self {
            system: System::new(config.system),
            jit: JIT::new(config.jit),
        }
    }

    /// Compiles a sequence of at most `limit` instructions starting at `addr` into a JIT block.
    fn compile(&mut self, addr: Address, limit: u32) -> ppcjit::Block {
        let _span = trace_span!("compiling new block", addr = ?self.system.cpu.pc).entered();

        let mut count = 0;
        let instructions = std::iter::from_fn(|| {
            if count >= limit {
                return None;
            }

            let current = addr + 4 * count;
            let physical = self.system.translate_instr_addr(current)?;

            let ins = Ins::new(self.system.bus.read(physical), Extensions::gekko_broadway());
            count += 1;

            Some(ins)
        });

        let block = self.jit.compiler.compile(instructions).unwrap();
        debug!(
            instructions = block.meta().seq.len(),
            "block sequence built"
        );

        block
    }

    #[inline(always)]
    fn exec_inner(&mut self, instr_limit: u32) -> Executed {
        let block = self
            .jit
            .blocks
            .mapping
            .get(self.system.cpu.pc)
            .and_then(|id| self.jit.blocks.storage.get(id))
            .filter(|b| b.meta().seq.len() <= instr_limit as usize);

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

        block.call(&mut ctx as *mut _ as *mut _, &CTX_HOOKS)
    }

    fn exec(&mut self, limits: Limits) -> Executed {
        let block = self
            .jit
            .blocks
            .mapping
            .get(self.system.cpu.pc)
            .and_then(|id| self.jit.blocks.storage.get(id));

        if block.is_none() {
            let block = self.compile(self.system.cpu.pc, self.jit.config.instr_per_block);
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

    fn closest_breakpoint(&self, breakpoints: &[Address]) -> Address {
        let mut closest_breakpoint = Address(self.system.cpu.pc.value().saturating_add(u32::MAX));
        let mut closest_distance = closest_breakpoint.value() - self.system.cpu.pc.value();
        for breakpoint in breakpoints.iter().copied() {
            let distance = breakpoint.value().checked_sub(self.system.cpu.pc.value());
            if let Some(distance) = distance
                && distance <= closest_distance
                && distance != 0
            {
                closest_breakpoint = breakpoint;
                closest_distance = distance;
            }
        }

        closest_breakpoint
    }

    #[inline(always)]
    fn run_inner<const BREAKPOINTS: bool>(
        &mut self,
        limits: Limits,
        breakpoints: &[Address],
    ) -> (Executed, bool) {
        let mut executed = Executed::default();
        let mut remaining_instr = limits.instructions;
        let mut remaining_cycles = limits.cycles;

        while remaining_cycles > 0 && remaining_instr > 0 {
            let until_next_event = self.system.scheduler.until_next().unwrap_or(u64::MAX);
            let cycles_to_run = until_next_event
                .min(remaining_cycles as u64)
                .min(u32::MAX as u64) as u32;

            let (instructions, closest_breakpoint) = if BREAKPOINTS {
                let closest_breakpoint = self.closest_breakpoint(breakpoints);
                let breakpoint_distance =
                    (closest_breakpoint.value() - self.system.cpu.pc.value()) / 4;

                (remaining_instr.min(breakpoint_distance), closest_breakpoint)
            } else {
                (remaining_instr, Address(0))
            };

            // let call_stack = self.system.call_stack();
            // if call_stack.0.len() > 0 {
            //     tracing::debug!("call stack:\n{call_stack}");
            // }

            let e = self.exec(Limits {
                cycles: cycles_to_run,
                instructions,
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

            if BREAKPOINTS && self.system.cpu.pc == closest_breakpoint {
                return (executed, true);
            }
        }

        (executed, false)
    }

    /// Runs the emulator respecting the given `limits`.
    pub fn run(&mut self, limits: Limits) -> Executed {
        self.run_inner::<false>(limits, &[]).0
    }

    /// Runs the emulator respecting the given `limits` and stopping at any address in `breakpoints`.
    /// Returns executed cycles/instructions and whether a breakpoint was hit.
    pub fn run_breakpoints(&mut self, limits: Limits, breakpoints: &[Address]) -> (Executed, bool) {
        self.run_inner::<true>(limits, breakpoints)
    }
}
