//! State of the emulator.

pub mod bus;
pub mod dsp;
pub mod mem;
pub mod mmu;
pub mod scheduler;
pub mod video;

use crate::system::{bus::Bus, mmu::Mmu, scheduler::Scheduler};
use common::{Address, arch::Cpu};
use dol::Dol;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Decrementer,
}

/// System state.
pub struct System {
    /// Scheduler for events.
    pub scheduler: Scheduler,
    /// The CPU state.
    pub cpu: Cpu,
    /// The system bus. Contains all other peripherals.
    pub bus: Bus,
    /// State of memory mapping.
    pub mmu: Mmu,
}

impl Default for System {
    fn default() -> Self {
        Self::new()
    }
}

impl System {
    pub fn new() -> Self {
        System {
            scheduler: Scheduler::default(),
            cpu: Cpu::default(),
            bus: Bus::new(),
            mmu: Mmu::new(),
        }
    }

    /// Translates a data logical address into a physical address.
    pub fn translate_data_addr(&self, addr: Address) -> Address {
        if !self.cpu.supervisor.config.msr.data_addr_translation() {
            return addr;
        }

        if let Some(addr) = self.mmu.translate_data_addr(addr) {
            return addr;
        }

        panic!("couldn't translate data addr {addr} with bats!")
    }

    /// Translates an instruction logical address into a physical address.
    pub fn translate_instr_addr(&self, addr: Address) -> Address {
        if !self.cpu.supervisor.config.msr.instr_addr_translation() {
            return addr;
        }

        if let Some(addr) = self.mmu.translate_instr_addr(addr) {
            return addr;
        }

        panic!("couldn't translate instruction addr {addr} with bats!")
    }

    /// Loads a `.dol` file.
    pub fn load(&mut self, dol: &Dol) {
        self.cpu.pc = Address(dol.entrypoint());

        self.cpu.supervisor.memory.setup_default_bats();
        self.mmu.build_bat_lut(&self.cpu.supervisor.memory);

        self.cpu
            .supervisor
            .config
            .msr
            .set_instr_addr_translation(true);
        self.cpu
            .supervisor
            .config
            .msr
            .set_data_addr_translation(true);

        // zero bss first, let others section overwrite it if it occurs
        for offset in 0..dol.header.bss_size {
            let target = self.translate_data_addr(Address(dol.header.bss_target + offset));
            self.bus.write(target, 0u8);
        }

        for section in dol.text_sections() {
            for (offset, byte) in section.content.iter().copied().enumerate() {
                let target = self.translate_instr_addr(Address(section.target) + offset as u32);
                self.bus.write(target, byte);
            }
        }

        for section in dol.data_sections() {
            for (offset, byte) in section.content.iter().copied().enumerate() {
                let target = self.translate_data_addr(Address(section.target) + offset as u32);
                self.bus.write(target, byte);
            }
        }
    }

    pub fn process(&mut self, event: Event) {
        tracing::info!("got {event:?}");
        self.scheduler.schedule(Event::Decrementer, 128);
    }
}
