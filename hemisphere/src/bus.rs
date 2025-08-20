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
            // --> VI
            0x0C00_2000, 2 => ll!(self.video.regs.vtr.as_bytes());
            0x0C00_2002, 2 => ll!(self.video.regs.dcr.as_bytes());
            0x0C00_2004, 8 => ll!(self.video.regs.htr.as_bytes());
            0x0C00_200C, 4 => ll!(self.video.regs.vto.as_bytes());
            0x0C00_2010, 4 => ll!(self.video.regs.vte.as_bytes());
            0x0C00_2014, 4 => ll!(self.video.regs.bbei.as_bytes());
            0x0C00_2018, 4 => ll!(self.video.regs.bboi.as_bytes());
            0x0C00_201C, 4 => ll!(self.video.regs.tfbl.as_bytes());
            0x0C00_2020, 4 => ll!(self.video.regs.tfbr.as_bytes());
            0x0C00_2024, 4 => ll!(self.video.regs.bfbl.as_bytes());
            0x0C00_2028, 4 => ll!(self.video.regs.bfbr.as_bytes());
            0x0C00_204A, 2 => ll!(self.video.regs.hsr.as_bytes());
            0x0C00_206C, 2 => ll!(self.video.regs.clk.as_bytes());
            0x0C00_2070, 2 => ll!(self.video.regs._2070.as_bytes());
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
            // --> VI
            0x0C00_2000, 2 => ll!(self.video.regs.vtr.as_mut_bytes());
            0x0C00_2002, 2 => ll!(self.video.regs.dcr.as_mut_bytes());
            0x0C00_2004, 8 => ll!(self.video.regs.htr.as_mut_bytes());
            0x0C00_200C, 4 => ll!(self.video.regs.vto.as_mut_bytes());
            0x0C00_2010, 4 => ll!(self.video.regs.vte.as_mut_bytes());
            0x0C00_2014, 4 => ll!(self.video.regs.bbei.as_mut_bytes());
            0x0C00_2018, 4 => ll!(self.video.regs.bboi.as_mut_bytes());
            0x0C00_201C, 4 => ll!(self.video.regs.tfbl.as_mut_bytes());
            0x0C00_2020, 4 => ll!(self.video.regs.tfbr.as_mut_bytes());
            0x0C00_2024, 4 => ll!(self.video.regs.bfbl.as_mut_bytes());
            0x0C00_2028, 4 => ll!(self.video.regs.bfbr.as_mut_bytes());
            0x0C00_204A, 2 => ll!(self.video.regs.hsr.as_mut_bytes());
            0x0C00_206C, 2 => ll!(self.video.regs.clk.as_mut_bytes());
            0x0C00_2070, 2 => ll!(self.video.regs._2070.as_mut_bytes());

            // FCT0 - FCT6 - stubbed, coefficients related to AA i guess?
            0x0C00_204C, 0x1A => ();
        }
    }
}
