//! Memory of the system.
use crate::system::ipl::Ipl;

pub const RAM_LEN: u32 = 24 * bytesize::MIB as u32;
pub const L2C_LEN: u32 = 16 * bytesize::KIB as u32;
pub const IPL_LEN: u32 = 2 * bytesize::MIB as u32;
pub const ARAM_LEN: u32 = 16 * bytesize::MIB as u32;
pub const SRAM_LEN: u32 = 64;

pub struct Memory {
    pub ram: Box<[u8; RAM_LEN as usize]>,
    pub l2c: Box<[u8; L2C_LEN as usize]>,
    pub ipl: Box<[u8; IPL_LEN as usize]>,
    pub aram: Box<[u8; ARAM_LEN as usize]>,
    pub sram: Box<[u8; SRAM_LEN as usize]>,
}

impl Memory {
    pub fn new(ipl: Ipl) -> Self {
        let mut ipl_mem = util::boxed_array(0);
        ipl_mem.copy_from_slice(&ipl);

        Self {
            ram: util::boxed_array(0),
            l2c: util::boxed_array(0),
            ipl: ipl_mem,
            aram: util::boxed_array(0),
            sram: util::boxed_array(0),
        }
    }
}
