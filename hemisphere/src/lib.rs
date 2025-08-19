pub mod bus;
pub mod jit;
pub mod mmu;

use crate::bus::Bus;
use dolfile::Dol;
use hemicore::{Address, Primitive};
use ppcjit::{
    Sequence, SequenceStatus,
    block::ExternalFunctions,
    powerpc::{Extensions, Ins},
};

pub use dolfile;
pub use hemicore;

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

fn external_functions() -> ExternalFunctions {
    extern "sysv64" fn read<T: Primitive>(
        bus: &mut Bus,
        registers: &ppcjit::Registers,
        addr: Address,
    ) -> T {
        let physical = registers.supervisor.translate_data_addr(addr);
        bus.read(physical)
    }

    extern "sysv64" fn write<T: Primitive>(
        bus: &mut Bus,
        registers: &ppcjit::Registers,
        addr: Address,
        value: T,
    ) {
        let physical = registers.supervisor.translate_data_addr(addr);
        bus.write(physical, value);
    }

    let read_i8 = unsafe { std::mem::transmute(read::<i8> as extern "sysv64" fn(_, _, _) -> _) };
    let write_i8 = unsafe { std::mem::transmute(write::<i8> as extern "sysv64" fn(_, _, _, _)) };
    let read_i16 = unsafe { std::mem::transmute(read::<i16> as extern "sysv64" fn(_, _, _) -> _) };
    let write_i16 = unsafe { std::mem::transmute(write::<i16> as extern "sysv64" fn(_, _, _, _)) };
    let read_i32 = unsafe { std::mem::transmute(read::<i32> as extern "sysv64" fn(_, _, _) -> _) };
    let write_i32 = unsafe { std::mem::transmute(write::<i32> as extern "sysv64" fn(_, _, _, _)) };

    ExternalFunctions {
        read_i8,
        write_i8,
        read_i16,
        write_i16,
        read_i32,
        write_i32,
    }
}

pub struct Hemisphere {
    pub bus: Bus,
    pub pc: Address,
    pub cpu: ppcjit::Registers,
    pub jit: ppcjit::JIT,
    pub blocks: jit::BlockStorage,
    pub config: Config,
}

impl Hemisphere {
    pub fn new(config: Config) -> Self {
        Self {
            bus: Bus::default(),
            pc: Address(0),
            cpu: ppcjit::Registers::default(),
            jit: ppcjit::JIT::default(),
            blocks: jit::BlockStorage::default(),
            config,
        }
    }

    pub fn load(&mut self, dol: &Dol) {
        self.pc = Address(dol.entrypoint());

        self.cpu.supervisor.msr.set_instr_addr_translation(true);
        self.cpu.supervisor.msr.set_data_addr_translation(true);
        self.cpu.supervisor.memory.setup_default_bats();

        for section in dol.text_sections() {
            let target = self
                .cpu
                .supervisor
                .translate_instr_addr(Address(section.target));

            for (i, byte) in section.content.iter().copied().enumerate() {
                self.bus.write(target + i as u32, byte);
            }
        }

        for section in dol.data_sections() {
            let target = self
                .cpu
                .supervisor
                .translate_data_addr(Address(section.target));

            for (i, byte) in section.content.iter().copied().enumerate() {
                self.bus.write(target + i as u32, byte);
            }
        }

        // TODO: zero BSS
    }

    /// Executes a single block and returns how many instructions were executed.
    pub fn exec(&mut self) -> u32 {
        let block = match self.blocks.get(self.pc) {
            Some(block) => block,
            None => {
                let mut seq = Sequence::new();
                let mut current = self.pc;

                loop {
                    if seq.len() >= self.config.instructions_per_block as usize {
                        break;
                    }

                    let physical = self.cpu.supervisor.translate_instr_addr(current);
                    let ins = Ins::new(self.bus.read(physical), Extensions::gekko_broadway());

                    let mut parsed = ppcjit::powerpc::ParsedIns::new();
                    ins.parse_basic(&mut parsed);

                    match seq.push(ins) {
                        Ok(SequenceStatus::Open) => current += 4,
                        _ => break,
                    }
                }

                let block = self.jit.build(seq).unwrap();
                let block = self.blocks.insert(self.pc, block).unwrap();

                block
            }
        };

        let funcs = external_functions();
        let output = block.run(&mut self.cpu, &mut self.bus as *mut _ as *mut _, &funcs);

        self.pc += 4 * output.executed;
        if output.jump.execute {
            if output.jump.link {
                self.cpu.user.lr = self.pc.0;
            }

            if output.jump.relative {
                self.pc += output.jump.data;
                self.pc -= 4;
            } else {
                self.pc = Address(output.jump.data as u32);
            }
        }

        output.executed
    }
}

#[test]
fn test() {
    let ins = Ins::new(0x80010000, ppcjit::powerpc::Extensions::gekko_broadway());
    let mut parsed = ppcjit::powerpc::ParsedIns::new();
    ins.parse_basic(&mut parsed);

    println!("{parsed}");
}
