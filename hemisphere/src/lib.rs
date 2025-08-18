pub mod bus;
pub mod jit;
pub mod mmu;

use crate::bus::Bus;
use hemicore::Address;
use ppcjit::{
    Sequence,
    powerpc::{Extensions, Ins},
};
use std::collections::hash_map::Entry;

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
    bus: Bus,
    pc: Address,
    cpu: ppcjit::Registers,
    jit: ppcjit::JIT,
    blocks: jit::BlockStorage,
    config: Config,
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

    /// Executes a single block and returns how many cycles have passed.
    pub fn exec(&mut self) -> u32 {
        match self.blocks.entry(self.pc) {
            Entry::Occupied(o) => {
                o.get().run(&mut self.cpu);
            }
            Entry::Vacant(v) => {
                let mut seq = Sequence::new();
                let mut current = self.pc;

                loop {
                    if seq.len() >= self.config.instructions_per_block as usize {
                        break;
                    }

                    let code = self.bus.read(current);
                    let ins = Ins::new(code, Extensions::gekko_broadway());
                    if seq.push(ins).is_err() {
                        break;
                    }

                    current += 1;
                }

                let block = self.jit.build(seq).unwrap();
                let block = v.insert(block);

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
