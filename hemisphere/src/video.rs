mod regs;

pub use regs::*;
use tinylog::Logger;

#[derive(Debug)]
pub struct VideoInterface {
    pub regs: Registers,
    pub logger: Logger,
}

impl VideoInterface {
    pub fn new(logger: Logger) -> Self {
        Self {
            regs: Registers::default(),
            logger,
        }
    }
}
