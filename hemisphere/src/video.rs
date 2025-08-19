#[repr(C)]
#[derive(Default)]
pub struct Registers {
    pub vtr: u16,
    pub dcr: u16,
    pub htr: u64,
}

#[derive(Default)]
pub struct VideoInterface {
    pub regs: Registers,
}
