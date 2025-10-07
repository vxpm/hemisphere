use bitos::BitUtils;
use common::util;

/// Transform unit
#[derive(Debug)]
pub struct Interface {
    pub ram: Box<[u32; 0x1000]>,
}

impl Default for Interface {
    fn default() -> Self {
        Self {
            ram: util::boxed_array(0),
        }
    }
}

impl Interface {
    pub fn write(&mut self, addr: u16, value: u32) {
        match addr {
            0x0000..0x0400 => self.ram[addr as usize] = value,
            0x0400..0x0460 => self.ram[addr as usize] = value.with_bits(0, 12, 0),
            0x0500..0x0600 => self.ram[addr as usize] = value,
            0x0600..0x0680 => self.ram[addr as usize] = value.with_bits(0, 12, 0),
            0x1000..0x1057 => {
                tracing::debug!("writing to internal XF register");
            }
            _ => tracing::debug!("writing to unknown XF memory"),
        }
    }
}
