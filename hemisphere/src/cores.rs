use crate::system::System;

pub struct Executed {
    pub instructions: u32,
    pub cycles: u32,
}

/// Limits for CPU core execution.
pub struct Limits {
    /// A hard-limit on how many instructions can be executed. This means that the number of
    /// executed instructions must always be less than or equal to this value.
    pub instructions: u32,
    /// A soft-limit on how many cycles can be executed. This means that the number of executed
    /// instructions can be less than this value or, at most, slightly above it.
    pub cycles: u32,
}

/// Trait for CPU cores.
pub trait CpuCore {
    /// Drives the CPU core forward within specific limits.
    fn exec(&mut self, sys: &mut System, limits: Limits) -> Executed;
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
