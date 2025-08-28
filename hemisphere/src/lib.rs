pub mod bus;
pub mod jit;
pub mod mmu;
pub mod video;

use crate::bus::Bus;
use dolfile::Dol;
use hemicore::{
    Address, Primitive,
    arch::{
        Registers,
        powerpc::{Extensions, Ins, ParsedIns},
    },
};
use ppcjit::{Sequence, SequenceStatus, block::ExternalFunctions};
use rustc_hash::FxHashSet;
use tracing::{info, info_span};

pub use dolfile;
pub use hemicore as core;

/// The CPU frequency.
pub const FREQUENCY: u64 = 486_000_000;

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

struct ExternalData<'a> {
    bus: &'a mut Bus,
    invalidated: &'a mut FxHashSet<Address>,
}

impl<'a> ExternalData<'a> {
    fn functions() -> ExternalFunctions {
        extern "sysv64" fn read<T: Primitive>(
            external: &mut ExternalData,
            registers: &Registers,
            addr: Address,
        ) -> T {
            let physical = registers.supervisor.translate_data_addr(addr);
            external.bus.read(physical)
        }

        extern "sysv64" fn write<T: Primitive>(
            external: &mut ExternalData,
            registers: &Registers,
            addr: Address,
            value: T,
        ) {
            external.invalidated.insert(addr);

            let physical = registers.supervisor.translate_data_addr(addr);
            external.bus.write(physical, value);
        }

        let read_i8 =
            unsafe { std::mem::transmute(read::<i8> as extern "sysv64" fn(_, _, _) -> _) };
        let write_i8 =
            unsafe { std::mem::transmute(write::<i8> as extern "sysv64" fn(_, _, _, _)) };
        let read_i16 =
            unsafe { std::mem::transmute(read::<i16> as extern "sysv64" fn(_, _, _) -> _) };
        let write_i16 =
            unsafe { std::mem::transmute(write::<i16> as extern "sysv64" fn(_, _, _, _)) };
        let read_i32 =
            unsafe { std::mem::transmute(read::<i32> as extern "sysv64" fn(_, _, _) -> _) };
        let write_i32 =
            unsafe { std::mem::transmute(write::<i32> as extern "sysv64" fn(_, _, _, _)) };

        ExternalFunctions {
            read_i8,
            write_i8,
            read_i16,
            write_i16,
            read_i32,
            write_i32,
        }
    }
}

pub struct Hemisphere {
    pub bus: Bus,
    pub cpu: Registers,
    pub jit: ppcjit::JIT,
    pub blocks: jit::BlockStorage,
    pub config: Config,
    invalidated: FxHashSet<Address>,
}

impl Hemisphere {
    pub fn new(config: Config) -> Self {
        Self {
            bus: Bus::new(),
            cpu: Registers::default(),
            jit: ppcjit::JIT::default(),
            blocks: jit::BlockStorage::default(),
            invalidated: FxHashSet::default(),
            config,
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

    fn compile(&mut self, addr: Address, limit: u16) -> ppcjit::Block {
        let _span = info_span!("compiling new block", addr = ?self.cpu.pc).entered();

        let mut seq = Sequence::new();
        let mut current = addr;

        loop {
            if seq.len() >= limit as usize {
                break;
            }

            let physical = self.cpu.supervisor.translate_instr_addr(current);
            let ins = Ins::new(self.bus.read(physical), Extensions::gekko_broadway());

            let mut parsed = ParsedIns::new();
            ins.parse_basic(&mut parsed);

            match seq.push(ins) {
                Ok(SequenceStatus::Open) => current += 4,
                _ => break,
            }
        }

        info!(instructions = seq.len(), "block sequence built");
        self.jit.compile(seq).unwrap()
    }

    fn exec_inner(&mut self, block: jit::BlockId) -> u32 {
        let block = self.blocks.get_by_id(block).unwrap();
        let mut external = ExternalData {
            bus: &mut self.bus,
            invalidated: &mut self.invalidated,
        };

        let funcs = ExternalData::functions();
        let output = block.run(&mut self.cpu, &mut external as *mut _ as *mut _, &funcs);

        for addr in self.invalidated.drain() {
            self.blocks.invalidate(addr);
        }

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

    /// Executes a single block with a limit of instructions and returns how many instructions were
    /// executed. This will _always_ compile a new block and it won't be cached in the storage.
    pub fn exec_limited(&mut self, limit: u16) -> u32 {
        let block = self.compile(self.cpu.pc, self.config.instructions_per_block);
        self.exec_inner(block)
    }

    /// Executes a single block and returns how many instructions were executed.
    pub fn exec(&mut self) -> u32 {
        let block = match self.blocks.get(self.cpu.pc) {
            Some(block) => block,
            None => {
                let block = self.compile(self.cpu.pc, self.config.instructions_per_block);
                self.blocks.insert(self.cpu.pc, block).unwrap()
            }
        };

        self.exec_inner(block)
    }
}
