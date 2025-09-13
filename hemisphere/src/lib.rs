//! Hemisphere: a Nintendo GameCube emulator

#![feature(cold_path)]

pub mod bus;
pub mod dsp;
pub mod jit;
pub mod mem;
pub mod mmu;
pub mod runner;
pub mod video;

use crate::{
    bus::Bus,
    jit::{CTX_HOOKS, Context, JIT},
    mmu::Mmu,
};
use common::arch::{
    Registers,
    disasm::{Extensions, Ins, ParsedIns},
};
use dol::Dol;
use ppcjit::{Sequence, SequenceStatus};
use tracing::{trace, trace_span};

pub use common::{self, Address, Primitive, arch};
pub use dol;

/// Emulator configuration.
pub struct Config {
    /// Maximum number of instructions per JIT block.
    pub instr_per_block: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            instr_per_block: 128,
        }
    }
}

/// System state.
pub struct System {
    pub cpu: Registers,
    pub bus: Bus,
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
            cpu: Registers::default(),
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

        for offset in 0..dol.header.bss_size {
            let target = self.translate_data_addr(Address(dol.header.bss_target + offset));
            self.bus.write(target, 0u8);
        }
    }
}

/// The Hemisphere emulator.
pub struct Hemisphere {
    pub config: Config,
    pub system: System,
    pub jit: JIT,
}

impl Hemisphere {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            system: System::new(),
            jit: JIT::new(),
        }
    }

    /// Compiles a sequence of at most `limit` instructions starting at `addr` into a JIT block.
    fn compile(&mut self, addr: Address, limit: u16) -> ppcjit::Block {
        let _span = trace_span!("compiling new block", addr = ?self.system.cpu.pc).entered();

        let mut seq = Sequence::new();
        let mut current = addr;

        loop {
            if seq.len() >= limit as usize {
                break;
            }

            let physical = self.system.translate_instr_addr(current);
            let ins = Ins::new(self.system.bus.read(physical), Extensions::gekko_broadway());

            let mut parsed = ParsedIns::new();
            ins.parse_basic(&mut parsed);

            match seq.push(ins) {
                Ok(SequenceStatus::Open) => {
                    if current == u32::MAX {
                        break;
                    } else {
                        current += 4
                    }
                }
                _ => break,
            }
        }

        trace!(instructions = seq.len(), "block sequence built");
        self.jit.compiler.compile(seq).unwrap()
    }

    pub fn exec_with_limit(&mut self, limit: u16) -> u32 {
        let block = self
            .jit
            .blocks
            .mapping
            .get(self.system.cpu.pc)
            .and_then(|id| self.jit.blocks.storage.get(id))
            .filter(|b| b.sequence().len() <= limit as usize);

        let compiled: ppcjit::Block;
        let block = match block {
            Some(block) => block,
            None => {
                std::hint::cold_path();

                compiled = self.compile(self.system.cpu.pc, limit);
                &compiled
            }
        };

        let mut ctx = Context {
            system: &mut self.system,
            mapping: &mut self.jit.blocks.mapping,
        };

        block.run(&mut ctx as *mut _ as *mut _, &CTX_HOOKS)
    }

    fn exec_with_limit_and_cached(&mut self, limit: u16) -> u32 {
        let block = self
            .jit
            .blocks
            .mapping
            .get(self.system.cpu.pc)
            .and_then(|id| self.jit.blocks.storage.get(id));

        if block.is_none() {
            let block = self.compile(self.system.cpu.pc, self.config.instr_per_block);
            self.jit.blocks.insert(self.system.cpu.pc, block);
        }

        self.exec_with_limit(limit)
    }

    /// Executes a single block and returns how many cycles were executed.
    pub fn exec(&mut self) -> u32 {
        // tracing::debug!("exec at {}", self.system.cpu.pc);

        let block = self
            .jit
            .blocks
            .mapping
            .get(self.system.cpu.pc)
            .and_then(|id| self.jit.blocks.storage.get(id));

        let block = match block {
            Some(block) => block,
            None => {
                std::hint::cold_path();

                let block = self.compile(self.system.cpu.pc, self.config.instr_per_block);
                let id = self.jit.blocks.insert(self.system.cpu.pc, block);

                self.jit.blocks.storage.get(id).unwrap()
            }
        };

        // tracing::debug!(
        //     "====> block seq:\n{}\n",
        //     block.sequence(),
        //     // block.clir(),
        // );

        let mut ctx = Context {
            system: &mut self.system,
            mapping: &mut self.jit.blocks.mapping,
        };

        block.run(&mut ctx as *mut _ as *mut _, &CTX_HOOKS)
    }
}
