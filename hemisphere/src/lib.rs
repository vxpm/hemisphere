#![feature(cold_path)]

mod primitive;
mod stream;

pub mod cores;
pub mod panic;
pub mod render;
pub mod system;

use crate::{cores::Cores, system::System};

pub use dol;
pub use gekko::{self, Address};
pub use iso;
pub use primitive::Primitive;

/// How many DSP cycles to execute per step.
const DSP_STEP: u64 = 512;

/// The Hemisphere emulator.
pub struct Hemisphere {
    /// System state.
    system: System,
    /// Cores of the emulator.
    cores: Cores,
}

impl Hemisphere {
    pub fn new(cores: Cores, config: system::Config) -> Self {
        Self {
            system: System::new(config),
            cores,
        }
    }

    /// Advances emulation by the specified number of CPU cycles.
    pub fn exec(&mut self, cycles: u32) -> cores::Executed {
        todo!()
    }
}
