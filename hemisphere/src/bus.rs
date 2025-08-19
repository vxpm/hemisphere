use crate::mmu::{Memory, RAM_LEN};
use hemicore::{Address, Primitive};

#[derive(Default)]
pub struct Bus {
    mem: Memory,
}

impl Bus {
    /// Reads a primitive from the given physical address.
    pub fn read<P: Primitive>(&self, addr: Address) -> P {
        match addr.value() {
            0..RAM_LEN => P::read_from_buf(&self.mem.ram[addr.value() as usize..]),
            _ => {
                println!("read from unimplemented address {addr}");
                P::default()
            }
        }
    }

    /// Writes a primitive to the given physical address.
    pub fn write<P: Primitive>(&mut self, addr: Address, value: P) {
        match addr.value() {
            0..RAM_LEN => value.write_to_buf(&mut self.mem.ram[addr.value() as usize..]),
            _ => println!("wrote to unimplemented address {} ({value:08X})", addr),
        }
    }
}
