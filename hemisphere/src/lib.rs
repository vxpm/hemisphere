//! # Hemisphere: a Nintendo GameCube emulator
//! This is the main crate of the hemisphere emulator (you could call it the core of the emulator).
//! The system state is defined in [`system`], with [`jit`] being glue-code to [`ppcjit`] and
//! [`runner`] being a way to setup emulation and drive it forward.

#![feature(cold_path)]

pub mod cores;
pub mod jit;
pub mod panic;
pub mod primitive;
pub mod render;
pub mod runner;
pub mod system;

use crate::{cores::Cores, system::System};

pub use dol;
pub use gekko::{self, Address};
pub use iso;

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
    /// Cores of the emulator.
    pub cores: Cores,
    /// System state.
    pub system: System,
}

impl Hemisphere {
    pub fn new(cores: Cores, config: system::Config) -> Self {
        Self {
            cores,
            system: System::new(config),
        }
    }

    fn exec(&mut self, limits: Limits) -> Executed {
        let block = self
            .jit
            .blocks
            .mapping
            .get(self.system.cpu.pc)
            .and_then(|id| self.jit.blocks.storage.get(id));

        if block.is_none() {
            // avoid trying to compile unimplemented instructions in debug mode
            let instructions = if cfg!(debug_assertions) {
                self.jit.config.instr_per_block.min(limits.instructions)
            } else {
                self.jit.config.instr_per_block
            };

            let block = self.compile(self.system.cpu.pc, instructions);
            self.jit.blocks.insert(self.system.cpu.pc, block);
        }

        self.exec_inner(limits.instructions)
    }

    pub fn step(&mut self) -> Executed {
        let executed = self.exec(Limits {
            instructions: 1,
            cycles: u32::MAX,
        });

        self.system.scheduler.advance(executed.cycles as u64);
        while let Some(event) = self.system.scheduler.pop() {
            self.system.process(event);
        }

        executed
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

            let instructions = if BREAKPOINTS {
                let closest_breakpoint = self.closest_breakpoint(breakpoints);
                let breakpoint_distance =
                    (closest_breakpoint.value() - self.system.cpu.pc.value()) / 4;

                remaining_instr.min(breakpoint_distance)
            } else {
                remaining_instr
            };

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

            if BREAKPOINTS && breakpoints.contains(&self.system.cpu.pc) {
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
