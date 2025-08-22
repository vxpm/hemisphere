mod regs;

pub use regs::*;

#[derive(Debug)]
pub struct VideoInterface {
    pub regs: Registers,
}

impl Default for VideoInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoInterface {
    pub fn new() -> Self {
        Self {
            regs: Registers::default(),
        }
    }
}
