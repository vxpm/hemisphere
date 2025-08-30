pub mod bus;
pub mod jit;
pub mod mmu;
pub mod video;

pub mod runner;

use crate::{
    bus::Bus,
    jit::{ExternalData, JIT},
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
            instructions_per_block: 64,
        }
    }
}

/// System state.
pub struct System {
    pub cpu: Registers,
    pub bus: Bus,
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
        self.cpu.supervisor.msr.set_instr_addr_translation(true);
        self.cpu.supervisor.msr.set_data_addr_translation(true);

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
    fn exec(&mut self, block: &ppcjit::Block, invalidated: &mut Vec<Address>) -> u32 {
        invalidated.clear();
        let mut external = ExternalData {
            bus: &mut self.bus,
            invalidated,
        };

        let funcs = ExternalData::functions();
        let output = block.run(&mut self.cpu, &mut external as *mut _ as *mut _, &funcs);

        self.cpu.pc += 4 * output.executed;
        if output.jump.execute {
            if output.jump.link {
                self.cpu.user.lr = self.cpu.pc.0;
            }

            if output.jump.relative {
                self.cpu.pc += output.jump.data;
                self.cpu.pc -= 4;
            } else {
                self.cpu.pc = Address(output.jump.data as u32);
            }
        }

        output.executed
    }
}

/// The Hemisphere emulator.
pub struct Hemisphere {
    pub config: Config,
    pub system: System,
    pub jit: JIT,
    invalidated: Vec<Address>,
}

impl Hemisphere {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            system: System::new(),
            jit: JIT::new(),
            invalidated: Vec::new(),
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
                Ok(SequenceStatus::Open) => current += 4,
                _ => break,
            }
        }

        info!(instructions = seq.len(), "block sequence built");
        self.jit.compiler.compile(seq).unwrap()
    }

    /// Executes a single block with a limit of instructions and returns how many instructions were
    /// executed. This will _always_ compile a new block and it won't be cached in the storage.
    pub fn exec_limited(&mut self, limit: u16) -> u32 {
        let block = self.compile(self.system.cpu.pc, limit);
        let executed = self.system.exec(&block, &mut self.invalidated);
        self.jit.blocks.invalidate(&self.invalidated);

        executed
    }

    /// Executes a single block and returns how many instructions were executed.
    pub fn exec(&mut self) -> u32 {
        let block = match self.jit.blocks.get(self.system.cpu.pc) {
            Some(block) => block,
            None => {
                let block = self.compile(self.system.cpu.pc, self.config.instructions_per_block);
                self.jit.blocks.insert(self.system.cpu.pc, block)
            }
        };

        let executed = self.system.exec(block, &mut self.invalidated);
        self.jit.blocks.invalidate(&self.invalidated);

        executed
    }
}
