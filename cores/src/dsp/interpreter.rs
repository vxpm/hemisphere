use dspint::Interpreter;
use hemisphere::{cores::DspCore, system::System};

pub struct InterpreterCore {
    interpreter: Interpreter,
}

impl DspCore for InterpreterCore {
    fn exec(&mut self, sys: &mut System, instructions: u32) -> u32 {
        for _ in 0..instructions {
            self.interpreter.step(sys);
        }

        instructions
    }
}
