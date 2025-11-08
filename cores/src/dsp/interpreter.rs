use hemisphere::{cores::DspCore, system::System};

pub struct InterpreterCore {}

impl DspCore for InterpreterCore {
    fn exec(&mut self, sys: &mut System, instructions: u32) -> u32 {
        todo!()
    }
}
