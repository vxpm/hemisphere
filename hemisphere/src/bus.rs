use crate::{
    mem::{Memory, RAM_LEN},
    video::VideoInterface,
};
use common::{Address, Primitive};
use tracing::{error, warn};
use zerocopy::IntoBytes;

/// The bus of the system. Contains all memory mapped peripherals.
pub struct Bus {
    pub mem: Memory,
    pub video: VideoInterface,
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus {
    pub fn new() -> Self {
        Self {
            mem: Memory::default(),
            video: VideoInterface::new(),
        }
    }
}

/// Allows the usage of const values in patterns. It's a neat trick!
struct ConstTrick<const N: u32>;
impl<const N: u32> ConstTrick<N> {
    const OUTPUT: u32 = N;
}

impl Bus {
    /// Reads a primitive from the given physical address, but only if it can't possibly have a
    /// side effect.
    pub fn read_pure<P: Primitive>(&self, addr: Address) -> Option<P> {
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
                        None
                    }
                }
            };
        }

        // read from big endian bytes
        macro_rules! be {
            ($bytes:expr) => {
                Some(P::read_be_bytes(&$bytes[offset..]))
            };
        }

        map! {
            // ADDR, SIZE => ACTION;
            0, RAM_LEN => be!(self.mem.ram.as_slice());
        }
    }

    /// Reads a primitive from the given physical address.
    pub fn read<P: Primitive>(&mut self, addr: Address) -> P {
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
                        error!("read from unimplemented address {addr}");
                        P::default()
                    }
                }
            };
        }

        // read from native endian bytes
        macro_rules! ne {
            ($bytes:expr) => {
                P::read_ne_bytes(&$bytes[offset..])
            };
        }

        // read from big endian bytes
        macro_rules! be {
            ($bytes:expr) => {
                P::read_be_bytes(&$bytes[offset..])
            };
        }

        map! {
            // ADDR, SIZE => ACTION;
            0, RAM_LEN => be!(self.mem.ram.as_slice());

            // === MMIO ===
            // --> VI
            0x0C00_2000, 2 => ne!(self.video.regs.vertical_timing.as_bytes());
            0x0C00_2002, 2 => ne!(self.video.regs.display_config.as_bytes());
            0x0C00_2004, 8 => ne!(self.video.regs.horizontal_timing.as_bytes());
            0x0C00_200C, 4 => ne!(self.video.regs.odd_field_vertical_timing.as_bytes());
            0x0C00_2010, 4 => ne!(self.video.regs.even_field_vertical_timing.as_bytes());
            0x0C00_2014, 4 => ne!(self.video.regs.odd_field_bb_interval.as_bytes());
            0x0C00_2018, 4 => ne!(self.video.regs.even_field_bb_interval.as_bytes());
            0x0C00_201C, 4 => ne!(self.video.regs.top_field_base.as_bytes());
            0x0C00_2020, 4 => ne!(self.video.regs.tfbr.as_bytes());
            0x0C00_2024, 4 => ne!(self.video.regs.bottom_field_base.as_bytes());
            0x0C00_2028, 4 => ne!(self.video.regs.bfbr.as_bytes());
            0x0C00_204A, 2 => ne!(self.video.regs.horizontal_scaling.as_bytes());
            0x0C00_206C, 2 => ne!(self.video.regs.clock.as_bytes());
            0x0C00_2070, 2 => ne!(self.video.regs._2070.as_bytes());
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
                        warn!("write to unimplemented address {addr} ({value:08X})");
                    }
                }
            };
        }

        // write as native endian bytes
        macro_rules! ne {
            ($bytes:expr) => {
                value.write_ne_bytes(&mut $bytes[offset..])
            };
        }

        // write as big endian bytes
        macro_rules! be {
            ($bytes:expr) => {
                value.write_be_bytes(&mut $bytes[offset..])
            };
        }

        map! {
            // ADDR, SIZE => ACTION;
            0, RAM_LEN => be!(self.mem.ram);

            // === MMIO ===
            // --> VI
            0x0C00_2000, 2 => ne!(self.video.regs.vertical_timing.as_mut_bytes());
            0x0C00_2002, 2 => ne!(self.video.regs.display_config.as_mut_bytes());
            0x0C00_2004, 8 => ne!(self.video.regs.horizontal_timing.as_mut_bytes());
            0x0C00_200C, 4 => ne!(self.video.regs.odd_field_vertical_timing.as_mut_bytes());
            0x0C00_2010, 4 => ne!(self.video.regs.even_field_vertical_timing.as_mut_bytes());
            0x0C00_2014, 4 => ne!(self.video.regs.odd_field_bb_interval.as_mut_bytes());
            0x0C00_2018, 4 => ne!(self.video.regs.even_field_bb_interval.as_mut_bytes());
            0x0C00_201C, 4 => ne!(self.video.regs.top_field_base.as_mut_bytes());
            0x0C00_2020, 4 => ne!(self.video.regs.tfbr.as_mut_bytes());
            0x0C00_2024, 4 => ne!(self.video.regs.bottom_field_base.as_mut_bytes());
            0x0C00_2028, 4 => ne!(self.video.regs.bfbr.as_mut_bytes());
            0x0C00_204A, 2 => ne!(self.video.regs.horizontal_scaling.as_mut_bytes());
            0x0C00_206C, 2 => ne!(self.video.regs.clock.as_mut_bytes());
            0x0C00_2070, 2 => ne!(self.video.regs._2070.as_mut_bytes());

            // FCT0 - FCT6 - stubbed, coefficients related to AA i guess?
            0x0C00_204C, 0x1A => ();
        }
    }
}
