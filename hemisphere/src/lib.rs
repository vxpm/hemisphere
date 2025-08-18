pub mod bus;
pub mod jit;
pub mod mmu;

use crate::bus::Bus;
use dolfile::Dol;
use hemicore::Address;
use ppcjit::{
    Sequence, SequenceStatus,
    block::Functions,
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

fn external_functions(bus: &mut Bus) -> Functions {
    extern "sysv64" fn read_i32(
        bus: &mut Bus,
        registers: &ppcjit::Registers,
        addr: Address,
    ) -> i32 {
        let physical = registers.supervisor.translate_data_addr(addr);
        bus.read(physical)
    }

    extern "sysv64" fn write_i32(
        bus: &mut Bus,
        registers: &ppcjit::Registers,
        addr: Address,
        value: i32,
    ) {
        println!("writing {value} to {addr}");
        let physical = registers.supervisor.translate_data_addr(addr);
        bus.write(physical, value);
    }

    let read_i32 = unsafe { std::mem::transmute(read_i32 as extern "sysv64" fn(_, _, _) -> _) };
    let write_i32 = unsafe { std::mem::transmute(write_i32 as extern "sysv64" fn(_, _, _, _)) };

    Functions {
        bus: bus as *mut _ as *mut _,
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

                    println!("{parsed}");

                    match seq.push(ins) {
                        Ok(SequenceStatus::Open) => current += 4,
                        _ => break,
                    }
                }

                let block = self.jit.build(seq).unwrap();
                let block = self.blocks.insert(self.pc, block).unwrap();

                println!("{}", block);

                block
            }
        };

        let funcs = external_functions(&mut self.bus);
        let output = block.run(&mut self.cpu, &funcs);

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
