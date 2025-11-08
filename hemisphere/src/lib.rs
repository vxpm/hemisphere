#![feature(cold_path)]

mod primitive;
mod stream;

pub mod cores;
pub mod panic;
pub mod render;
pub mod system;

use crate::{cores::Cores, system::System};

pub use dol;
pub use gekko::{self, Address, Cycles};
pub use iso;
pub use primitive::Primitive;

/// How many DSP cycles to execute per step.
const DSP_STEP: u32 = 512;

/// The Hemisphere emulator.
pub struct Hemisphere {
    /// System state.
    system: System,
    /// Cores of the emulator.
    cores: Cores,
    /// How many DSP cycles are pending.
    dsp_pending: f64,
}

impl Hemisphere {
    pub fn new(cores: Cores, config: system::Config) -> Self {
        Self {
            system: System::new(config),
            cores,
            dsp_pending: 0.0,
        }
    }

    /// Advances emulation by the specified number of CPU cycles.
    pub fn exec(&mut self, cycles: Cycles) -> cores::Executed {
        let mut executed = cores::Executed::default();
        while executed.cycles < cycles {
            // how many CPU cycles can we execute?
            let remaining = cycles - executed.cycles;
            let until_next_dsp_step =
                Cycles((6.0 * ((DSP_STEP as f64) - self.dsp_pending)).ceil() as u64);
            let can_execute = until_next_dsp_step.min(remaining);

            let e = self.cores.cpu.exec(&mut self.system, can_execute, &[]);
            executed.instructions += e.instructions;
            executed.cycles += e.cycles;
            self.dsp_pending += e.cycles.to_dsp_cycles();

            while self.dsp_pending > DSP_STEP as f64 {
                self.cores.dsp.exec(&mut self.system, DSP_STEP);
                self.dsp_pending %= DSP_STEP as f64;
            }
        }

        executed
    }
}
