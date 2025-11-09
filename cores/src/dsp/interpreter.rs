use dspint::Interpreter;
use hemisphere::{cores::DspCore, system::System};

#[derive(Default)]
pub struct InterpreterCore {
    interpreter: Interpreter,
}

impl DspCore for InterpreterCore {
    fn exec(&mut self, sys: &mut System, instructions: u32) -> u32 {
        self.interpreter.do_dma(sys);
        self.interpreter.check_reset(sys);

        for _ in 0..instructions {
            self.interpreter.step(sys);
        }

        instructions
    }
}
