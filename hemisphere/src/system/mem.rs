use common::util::boxed_array;

pub const RAM_LEN: u32 = 24 * bytesize::MIB as u32;
pub const EFB_LEN: u32 = 2 * bytesize::MIB as u32;
pub const L2C_LEN: u32 = 16 * bytesize::KIB as u32;
pub const IPL_LEN: u32 = bytesize::MIB as u32;
pub const ARAM_LEN: u32 = 16 * bytesize::MIB as u32;

pub struct Memory {
    pub ram: Box<[u8; RAM_LEN as usize]>,
    pub aram: Box<[u8; ARAM_LEN as usize]>,
    pub efb: Box<[u8; EFB_LEN as usize]>,
    pub l2c: Box<[u8; L2C_LEN as usize]>,
    pub ipl: Box<[u8; IPL_LEN as usize]>,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            ram: boxed_array(0xFA),
            aram: boxed_array(0xFA),
            efb: boxed_array(0xFA),
            l2c: boxed_array(0xFA),
            ipl: boxed_array(0xFA),
        }
    }
}
