use super::{DSP_COEF, DSP_ROM};
use dspint::Interpreter;
use hemisphere::{cores::DspCore, system::System};

pub struct InterpreterCore {
    interpreter: Interpreter,
}

impl Default for InterpreterCore {
    fn default() -> Self {
        let mut interpreter = Interpreter::default();
        interpreter.mem.irom.copy_from_slice(&DSP_ROM[..]);
        interpreter.mem.coef.copy_from_slice(&DSP_COEF[..]);

        Self { interpreter }
    }
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
