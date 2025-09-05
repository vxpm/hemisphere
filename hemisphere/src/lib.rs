#![feature(cold_path)]

pub mod bus;
pub mod jit;
pub mod mem;
pub mod runner;
pub mod video;

use crate::{
    bus::Bus,
    jit::{EXTERNAL_FUNCTIONS, ExternalData, JIT},
};
use dolfile::Dol;
use hemicore::{
    Address,
    arch::{
        Registers,
        powerpc::{Extensions, Ins, ParsedIns},
    },
};
use ppcjit::{Sequence, SequenceStatus};
use tracing::{info, info_span};

pub use dolfile;
pub use hemicore as core;

/// The CPU frequency.
pub const FREQUENCY: u32 = 486_000_000;

/// Emulator configuration.
pub struct Config {
    /// Maximum number of instructions per JIT block.
    pub instructions_per_block: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            instructions_per_block: 128,
        }
    }
}

/// System state.
pub struct System {
    pub cpu: Registers,
    pub bus: Bus,
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
        }
    }

    pub fn load(&mut self, dol: &Dol) {
        self.cpu.pc = Address(dol.entrypoint());
        self.cpu.supervisor.memory.setup_default_bats();
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
                let target = self
                    .cpu
                    .supervisor
                    .translate_instr_addr(Address(section.target) + offset as u32);

                self.bus.write(target, byte);
            }
        }

        for section in dol.data_sections() {
            for (offset, byte) in section.content.iter().copied().enumerate() {
                let target = self
                    .cpu
                    .supervisor
                    .translate_data_addr(Address(section.target) + offset as u32);

                self.bus.write(target, byte);
            }
        }

        for offset in 0..dol.header.bss_size {
            let target = self
                .cpu
                .supervisor
                .translate_data_addr(Address(dol.header.bss_target + offset));

            self.bus.write(target, 0u8);
        }
    }

    /// Executes the given block on this state and fills `invalidated` with addresses that have
    /// been written to.
    #[inline(always)]
    fn exec(&mut self, block: &ppcjit::Block, invalidated: &mut Vec<Address>) -> u32 {
        // SAFETY: invalidated is a Vec of (Address)es, which are simple wrappers around u32s
        unsafe { invalidated.set_len(0) };

        let mut external = ExternalData {
            bus: &mut self.bus,
            invalidated,
        };

        block.run(
            &mut self.cpu,
            &mut external as *mut _ as *mut _,
            &EXTERNAL_FUNCTIONS,
        )
    }
}

/// The Hemisphere emulator.
pub struct Hemisphere {
    pub config: Config,
    pub system: System,
    pub jit: JIT,
    invalidated: Vec<Address>,
    last_limited_block: Option<(Address, ppcjit::Block)>,
}

impl Hemisphere {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            system: System::new(),
            jit: JIT::new(),
            invalidated: Vec::new(),
            last_limited_block: None,
        }
    }

    fn compile(&mut self, addr: Address, limit: u16) -> ppcjit::Block {
        let _span = info_span!("compiling new block", addr = ?self.system.cpu.pc).entered();

        let mut seq = Sequence::new();
        let mut current = addr;

        loop {
            if seq.len() >= limit as usize {
                break;
            }

            let physical = self.system.cpu.supervisor.translate_instr_addr(current);
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

        info!(instructions = seq.len(), "block sequence built");
        self.jit.compiler.compile(seq).unwrap()
    }

    #[inline(always)]
    fn invalidate(&mut self) {
        if self.invalidated.is_empty() {
            return;
        }

        for &invalidated in &self.invalidated {
            self.jit.blocks.invalidate(invalidated);

            if self
                .last_limited_block
                .as_ref()
                .is_some_and(|(addr, _)| *addr == invalidated)
            {
                std::hint::cold_path();
                self.last_limited_block = None;
            }
        }

        self.invalidated.clear();
    }

    /// Executes a single block with a limit of instructions and returns how many instructions were
    /// executed.
    pub fn exec_with_limit(&mut self, limit: u16) -> u32 {
        let block = if let Some(in_storage) = self.jit.blocks.get(self.system.cpu.pc)
            && in_storage.sequence().len() <= limit as usize
        {
            in_storage
        } else if let Some((addr, block)) = &self.last_limited_block
            && *addr == self.system.cpu.pc
        {
            block
        } else {
            std::hint::cold_path();
            let compiled = self.compile(self.system.cpu.pc, limit);
            self.last_limited_block = Some((self.system.cpu.pc, compiled));
            self.last_limited_block.as_ref().map(|(_, b)| b).unwrap()
        };

        let executed = self.system.exec(&block, &mut self.invalidated);
        self.invalidate();

        executed
    }

    /// Executes a single block with a limit of instructions and returns how many instructions were
    /// executed.
    fn exec_with_limit_and_cached(&mut self, limit: u16) -> u32 {
        if self.jit.blocks.get(self.system.cpu.pc).is_none() {
            let block = self.compile(self.system.cpu.pc, self.config.instructions_per_block);
            self.jit.blocks.insert(self.system.cpu.pc, block);
        }

        self.exec_with_limit(limit)
    }

    /// Executes a single block and returns how many instructions were executed.
    pub fn exec(&mut self) -> u32 {
        let block = match self.jit.blocks.get(self.system.cpu.pc) {
            Some(block) => block,
            None => {
                std::hint::cold_path();

                let block = self.compile(self.system.cpu.pc, self.config.instructions_per_block);
                self.jit.blocks.insert(self.system.cpu.pc, block)
            }
        };

        let executed = self.system.exec(block, &mut self.invalidated);
        self.invalidate();

        executed
    }
}
