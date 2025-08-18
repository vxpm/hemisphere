pub mod bus;
pub mod jit;
pub mod mmu;

use crate::bus::Bus;
use dolfile::Dol;
use hemicore::Address;
use ppcjit::{
    Sequence, SequenceStatus,
    powerpc::{Extensions, Ins},
};

pub use dolfile;

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

    /// Translates an instruction effective address into a physical address.
    pub fn translate_instr_addr(&self, addr: Address) -> Address {
        if !self.cpu.supervisor.msr.instr_addr_translation() {
            return addr;
        }

        for bat in &self.cpu.supervisor.memory.ibat {
            if bat.contains(addr) {
                return bat.translate(addr);
            }
        }

        panic!("couldn't translate instr addr with bats!")
    }

    /// Translates a data effective address into a physical address.
    pub fn translate_data_addr(&self, addr: Address) -> Address {
        if !self.cpu.supervisor.msr.data_addr_translation() {
            return addr;
        }

        for bat in &self.cpu.supervisor.memory.dbat {
            if bat.contains(addr) {
                return bat.translate(addr);
            }
        }

        panic!("couldn't translate instr addr with bats!")
    }

    pub fn load(&mut self, dol: &Dol) {
        self.pc = Address(dol.entrypoint());

        self.cpu.supervisor.msr.set_instr_addr_translation(true);
        self.cpu.supervisor.msr.set_data_addr_translation(true);
        self.cpu.supervisor.memory.setup_default_bats();

        for section in dol.text_sections() {
            let target = self.translate_instr_addr(Address(section.target));
            for (i, byte) in section.content.iter().copied().enumerate() {
                self.bus.write(target + i as u32, byte);
            }
        }
    }

    /// Executes a single block and returns how many cycles have passed.
    pub fn exec(&mut self) -> u32 {
        match self.blocks.get(self.pc) {
            Some(_) => todo!(),
            None => {
                let mut seq = Sequence::new();
                let mut current = self.pc;

                loop {
                    if seq.len() >= self.config.instructions_per_block as usize {
                        break;
                    }

                    let physical = self.translate_instr_addr(current);
                    let ins = Ins::new(self.bus.read(physical), Extensions::gekko_broadway());

                    match seq.push(ins) {
                        Ok(SequenceStatus::Open) => current += 4,
                        _ => break,
                    }
                }

                let block = self.jit.build(seq).unwrap();
                let block = self.blocks.insert(self.pc, block).unwrap();

                block.run(&mut self.cpu);
            }
        }

        // stub
        1
    }
}

#[cfg(test)]
mod test {
    use bitos::integer::{u11, u15};
    use hemicore::Address;
    use ppcjit::registers::Bat;

    pub fn translate(bats: &[Bat; 4], addr: Address) -> Option<Address> {
        for bat in bats {
            if (bat.start()..=bat.end()).contains(&addr) {
                return Some(bat.translate(addr));
            }
        }

        None
    }

    #[test]
    fn test() {
        let a = Bat::default()
            .with_effective_page_index(u15::new(0))
            .with_real_page_number(u15::new(0xFF00))
            .with_block_length_mask(u11::new(0x0000));

        dbg!(bytesize::ByteSize(a.block_length() as u64));
        dbg!(a.start()..a.end());
        dbg!(a.physical_start()..a.physical_end());

        let b = Bat::default()
            .with_effective_page_index(u15::new(1))
            .with_real_page_number(u15::new(0xFF00))
            .with_block_length_mask(u11::new(0x0000));

        dbg!(bytesize::ByteSize(b.block_length() as u64));
        dbg!(b.start()..b.end());
        dbg!(b.physical_start()..b.physical_end());

        let bats = [a, b, Bat::default(), Bat::default()];
        dbg!(translate(&bats, Address(0x1FFFF + 1)));
    }
}
