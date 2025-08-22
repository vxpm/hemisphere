mod regs;

pub use regs::*;

#[derive(Debug)]
pub struct VideoInterface {
    pub regs: Registers,
}

impl VideoInterface {
    pub fn new() -> Self {
        Self {
            regs: Registers::default(),
        }
    }
}
