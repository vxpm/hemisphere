use crate::system::System;
use gekko::Address;

#[derive(Default, Clone, Copy)]
pub struct Executed {
    /// How many instructions have been executed.
    pub instructions: u32,
    /// How many cycles have been executed.
    pub cycles: u32,
    /// Whether a breakpoint was hit.
    pub hit_breakpoint: bool,
}

/// Trait for CPU cores.
pub trait CpuCore {
    /// Drives the CPU core forward by approximatedly the given number of `cycles`, stopping at any
    /// address in `breakpoints`.
    fn exec(&mut self, sys: &mut System, cycles: u32, breakpoints: &[Address]) -> Executed;
}

/// Trait for DSP cores.
pub trait DspCore {
    /// Drives the DSP core forward by _at most_ the specified amount of instructions. The actual
    /// number of instructions executed is returned.
    fn exec(&mut self, sys: &mut System, instructions: u32) -> u32;
}

/// Cores that emulate system components.
pub struct Cores {
    pub cpu: Box<dyn CpuCore>,
    pub dsp: Box<dyn DspCore>,
}
