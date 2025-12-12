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

        if sys.dsp.control.halt()
            || !sys.dsp.cpu_mailbox.status() && self.interpreter.is_waiting_for_cpu_mail()
            || sys.dsp.dsp_mailbox.status() && self.interpreter.is_waiting_for_dsp_mail()
        {
            std::hint::cold_path();
            self.interpreter.check_exceptions(sys);
        } else {
            let mut i = 0;
            while i < instructions {
                i += 1;
                self.interpreter.step(sys);
            }
        }

        instructions
    }
}
