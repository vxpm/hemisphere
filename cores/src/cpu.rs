use hemisphere::{
    Address, Cycles,
    cores::{CpuCore, Executed},
    system::System,
};

pub struct JitCore {}

impl CpuCore for JitCore {
    fn exec(&mut self, sys: &mut System, cycles: Cycles, breakpoints: &[Address]) -> Executed {
        todo!()
    }
}
