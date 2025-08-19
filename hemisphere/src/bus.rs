use crate::{
    mmu::{Memory, RAM_LEN},
    video::VideoInterface,
};
use hemicore::{Address, Primitive};
use zerocopy::IntoBytes;

#[derive(Default)]
pub struct Bus {
    pub mem: Memory,
    pub video: VideoInterface,
}

struct ConstTrick<const N: u32>;
impl<const N: u32> ConstTrick<N> {
    const OUTPUT: u32 = N;
}

impl Bus {
    /// Reads a primitive from the given physical address.
    pub fn read<P: Primitive>(&self, addr: Address) -> P {
        let offset: usize;
        macro_rules! map {
            ($($addr:expr, $size:expr => $block:expr);* $(;)?) => {
                match addr.value() {
                    $(
                        $addr..ConstTrick::<{ $addr + $size }>::OUTPUT => {
                            #[allow(unused_assignments)]
                            {
                                offset = (addr.value() - $addr) as usize;
                            }
                            $block
                        }
                    )*
                    _ => {
                        println!("read from unimplemented address {addr}");
                        P::default()
                    }
                }
            };
        }

        macro_rules! ll {
            ($bytes:expr) => {
                P::read_le_bytes(&$bytes[offset..])
            };
        }

        macro_rules! bb {
            ($bytes:expr) => {
                P::read_be_bytes(&$bytes[offset..])
            };
        }

        map! {
            // ADDR, SIZE => ACTION;
            0, RAM_LEN => bb!(self.mem.ram.as_slice());

            // === MMIO ===
            // VI
            0x0C00_2000, 2 => ll!(self.video.regs.vtr.as_bytes());
            0x0C00_2002, 2 => ll!(self.video.regs.dcr.as_bytes());
            0x0C00_2004, 4 => ll!(self.video.regs.htr.as_bytes());
        }
    }

    /// Writes a primitive to the given physical address.
    pub fn write<P: Primitive>(&mut self, addr: Address, value: P) {
        let offset: usize;
        macro_rules! map {
            ($($addr:expr, $size:expr => $block:expr);* $(;)?) => {
                match addr.value() {
                    $(
                        $addr..ConstTrick::<{ $addr + $size }>::OUTPUT => {
                            #[allow(unused_assignments)]
                            {
                                offset = (addr.value() - $addr) as usize;
                            }
                            $block
                        }
                    )*
                    _ => {
                        panic!("write to unimplemented address {addr} ({value:08X})");
                    }
                }
            };
        }

        macro_rules! ll {
            ($bytes:expr) => {
                value.write_le_bytes(&mut $bytes[offset..])
            };
        }

        macro_rules! bb {
            ($bytes:expr) => {
                value.write_be_bytes(&mut $bytes[offset..])
            };
        }

        map! {
            // ADDR, SIZE => ACTION;
            0, RAM_LEN => bb!(self.mem.ram);

            // === MMIO ===
            // VI
            0x0C00_2000, 2 => ll!(self.video.regs.vtr.as_mut_bytes());
            0x0C00_2002, 2 => ll!(self.video.regs.dcr.as_mut_bytes());
            0x0C00_2004, 4 => ll!(self.video.regs.htr.as_mut_bytes());
        }
    }
}
