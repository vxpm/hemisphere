use crate::system::System;
use gekko::{Address, Cycles};

#[derive(Default, Clone, Copy)]
pub struct Executed {
    /// How many instructions have been executed.
    pub instructions: u32,
    /// How many cycles have been executed.
    pub cycles: Cycles,
    /// Whether a breakpoint was hit.
    pub hit_breakpoint: bool,
}

/// Trait for CPU cores.
pub trait CpuCore: Send {
    /// Drives the CPU core forward by approximatedly the given number of `cycles`, stopping at any
    /// address in `breakpoints`.
    fn exec(&mut self, sys: &mut System, cycles: Cycles, breakpoints: &[Address]) -> Executed;
    /// Steps the CPU, i.e. runs exactly 1 instruction.
    fn step(&mut self, sys: &mut System) -> Executed;
}

/// Cores that emulate system components.
pub struct Cores {
    pub cpu: Box<dyn CpuCore>,
}
