use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};

use super::{DSP_COEF, DSP_ROM};
use dspint::{Interpreter, Mmio};
use hemisphere::modules::dsp::{DspModule, DspMut, DspRef};

pub struct Module {
    interpreter: Interpreter,
}

impl Default for Module {
    fn default() -> Self {
        let mut interpreter = Interpreter::default();
        interpreter.mem.irom.copy_from_slice(&DSP_ROM[..]);
        interpreter.mem.coef.copy_from_slice(&DSP_COEF[..]);

        Self { interpreter }
    }
}

impl DspModule for Module {
    fn prepare(&mut self, ram: &mut [u8]) {
        self.interpreter.do_dma(ram);
        self.interpreter.check_reset(ram);
    }

    fn exec(&mut self, instructions: u32) -> u32 {
        let idle = {
            let mmio = self.interpreter.mmio.get();
            mmio.control.halt()
                || !mmio.cpu_mailbox.status() && self.interpreter.is_waiting_for_cpu_mail()
                || mmio.dsp_mailbox.status() && self.interpreter.is_waiting_for_dsp_mail()
        };

        if idle {
            std::hint::cold_path();
            self.interpreter.check_interrupts();
        } else {
            self.interpreter.exec(instructions);
        }

        instructions
    }

    fn state(&self) -> DspRef<'_> {
        DspRef::RwLock(self.interpreter.mmio.get())
    }

    fn state_mut(&mut self) -> DspMut<'_> {
        DspMut::RwLock(self.interpreter.mmio.get_mut())
    }
}

fn worker(pending_execution: Arc<AtomicU32>, interpreter: Arc<Mutex<Interpreter>>) {
    loop {
        let to_exec = pending_execution.load(Ordering::Acquire);
        if to_exec == 0 {
            atomic_wait::wait(&pending_execution, 0);
            continue;
        }

        let mut interpreter = interpreter.lock().unwrap();
        interpreter.exec(to_exec);
        std::mem::drop(interpreter);

        pending_execution.store(0, Ordering::Release);
    }
}

pub struct ThreadedModule {
    mmio: Mmio,
    interpreter: Arc<Mutex<Interpreter>>,
    pending_execution: Arc<AtomicU32>,
}

impl Default for ThreadedModule {
    fn default() -> Self {
        let mut interpreter = Interpreter::default();
        interpreter.mem.irom.copy_from_slice(&DSP_ROM[..]);
        interpreter.mem.coef.copy_from_slice(&DSP_COEF[..]);

        let mmio = interpreter.mmio.clone();
        let interpreter = Arc::new(Mutex::new(interpreter));
        let pending_execution = Arc::new(AtomicU32::new(0));

        std::thread::Builder::new()
            .name("dsp runner".into())
            .spawn({
                let interpreter = interpreter.clone();
                let pending_execution = pending_execution.clone();
                move || worker(pending_execution, interpreter)
            })
            .unwrap();

        Self {
            mmio,
            interpreter,
            pending_execution,
        }
    }
}

impl DspModule for ThreadedModule {
    fn prepare(&mut self, ram: &mut [u8]) {
        let mut interpreter = self.interpreter.lock().unwrap();
        interpreter.do_dma(ram);
        interpreter.check_reset(ram);
    }

    fn exec(&mut self, instructions: u32) -> u32 {
        std::mem::drop(self.interpreter.lock().unwrap());
        self.pending_execution
            .store(instructions, Ordering::Relaxed);
        atomic_wait::wake_all(&*self.pending_execution);

        instructions
    }

    fn state(&self) -> DspRef<'_> {
        DspRef::RwLock(self.mmio.get())
    }

    fn state_mut(&mut self) -> DspMut<'_> {
        DspMut::RwLock(self.mmio.get_mut())
    }
}
