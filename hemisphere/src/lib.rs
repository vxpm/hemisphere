#![feature(cold_path)]

mod primitive;
mod stream;

pub mod cores;
pub mod modules;

pub mod panic;
pub mod system;

use crate::{
    cores::Cores,
    system::{Modules, System},
};

pub use dol;
pub use gekko::{self, Address, Cycles};
pub use iso;
pub use primitive::Primitive;

/// How many DSP cycles to execute per step.
const DSP_STEP: u32 = 256;

/// The Hemisphere emulator.
pub struct Hemisphere {
    /// System state.
    pub system: System,
    /// Cores of the emulator.
    cores: Cores,
    /// How many DSP cycles are pending.
    dsp_pending: f64,
}

impl Hemisphere {
    pub fn new(cores: Cores, modules: Modules, config: system::Config) -> Self {
        Self {
            system: System::new(modules, config),
            cores,
            dsp_pending: 0.0,
        }
    }

    /// Advances emulation by the specified number of CPU cycles.
    pub fn exec(&mut self, cycles: Cycles, breakpoints: &[Address]) -> cores::Executed {
        let mut executed = cores::Executed::default();
        while executed.cycles < cycles {
            // how many CPU cycles can we execute?
            let remaining = cycles - executed.cycles;
            let until_next_dsp_step =
                Cycles((6.0 * ((DSP_STEP as f64) - self.dsp_pending)).ceil() as u64);
            let until_next_event = Cycles(self.system.scheduler.until_next().unwrap_or(u64::MAX));
            let can_execute = until_next_dsp_step.min(until_next_event).min(remaining);

            // execute CPU
            let e = self
                .cores
                .cpu
                .exec(&mut self.system, can_execute, breakpoints);
            executed.instructions += e.instructions;
            executed.cycles += e.cycles;
            executed.hit_breakpoint = e.hit_breakpoint;

            // execute DSP
            self.dsp_pending += e.cycles.to_dsp_cycles();
            while self.dsp_pending >= DSP_STEP as f64 {
                self.cores.dsp.exec(&mut self.system, DSP_STEP);
                self.dsp_pending -= DSP_STEP as f64;
            }

            self.system.scheduler.advance(e.cycles.0);
            self.system.process_events();

            if e.hit_breakpoint {
                std::hint::cold_path();
                break;
            }
        }

        executed
    }

    pub fn step(&mut self) -> cores::Executed {
        // execute CPU
        let executed = self.cores.cpu.step(&mut self.system);
        self.dsp_pending += executed.cycles.to_dsp_cycles();

        // execute DSP
        while self.dsp_pending >= DSP_STEP as f64 {
            self.cores.dsp.exec(&mut self.system, DSP_STEP);
            self.dsp_pending -= DSP_STEP as f64;
        }

        // process events
        self.system.scheduler.advance(executed.cycles.0);
        self.system.process_events();

        executed
    }
}
