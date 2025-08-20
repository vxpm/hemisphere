mod regs;

pub use regs::*;

#[derive(Debug, Default)]
pub struct VideoInterface {
    pub regs: Registers,
}
