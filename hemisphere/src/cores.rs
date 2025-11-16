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
pub trait CpuCore {
    /// Drives the CPU core forward by approximatedly the given number of `cycles`, stopping at any
    /// address in `breakpoints`.
    fn exec(&mut self, sys: &mut System, cycles: Cycles, breakpoints: &[Address]) -> Executed;
    /// Steps the CPU, i.e. runs exactly 1 instruction.
    fn step(&mut self, sys: &mut System) -> Executed;
}

/// Trait for DSP cores.
pub trait DspCore {
    /// Drives the DSP core forward by _at most_ the specified amount of instructions. The actual
    /// number of instructions executed is returned.
    fn exec(&mut self, sys: &mut System, instructions: u32) -> u32;
}

#[derive(Debug, Clone, Copy)]
pub struct ControllerState {
    // Analog
    pub analog_x: u8,
    pub analog_y: u8,
    pub analog_sub_x: u8,
    pub analog_sub_y: u8,

    // Analog Triggers
    pub analog_trigger_left: u8,
    pub analog_trigger_right: u8,

    // Digital Triggers
    pub trigger_z: bool,
    pub trigger_right: bool,
    pub trigger_left: bool,

    // Pad
    pub pad_left: bool,
    pub pad_right: bool,
    pub pad_down: bool,
    pub pad_up: bool,

    // Buttons
    pub button_a: bool,
    pub button_b: bool,
    pub button_x: bool,
    pub button_y: bool,
    pub button_start: bool,
}

/// Trait for input cores.
pub trait InputCore {
    fn controller(&mut self, index: usize) -> Option<ControllerState>;
}

/// Cores that emulate system components.
pub struct Cores {
    pub cpu: Box<dyn CpuCore>,
    pub dsp: Box<dyn DspCore>,
    pub input: Box<dyn InputCore>,
}
