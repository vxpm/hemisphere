use crate::{Limits, runner::Shared};
use color_backtrace::BacktracePrinter;
use common::arch::FREQUENCY;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const STEP_SIZE: u32 = FREQUENCY / 8000; // 1/8 ms

#[inline(always)]
fn to_duration(cycles: u32) -> Duration {
    Duration::from_secs_f64(cycles as f64 / FREQUENCY as f64)
}

fn setup_panic_hook() {
    crate::panic::set_hook(
        Box::new(move |info| {
            let backtrace = backtrace::Backtrace::new();
            let message = info.payload_as_str().unwrap_or("(unknown)");

            let mut buffer = color_backtrace::termcolor::Buffer::no_color();
            _ = BacktracePrinter::new()
                .message(message)
                .print_trace(&backtrace, &mut buffer);

            tracing::error!(
                "runner panicked: {message}\n{}",
                String::from_utf8_lossy(buffer.as_slice())
            );
        }),
        false,
    );
}

pub struct WorkerThread {
    state: Arc<Shared>,
}

impl WorkerThread {
    pub fn new(state: Arc<Shared>) -> Self {
        Self { state }
    }

    pub fn run(self) {
        setup_panic_hook();

        let mut next = Instant::now();
        'outer: loop {
            // wait until its ok to run
            while !self.state.advance.get() {
                std::hint::spin_loop();
                std::thread::yield_now();

                next = next.max(Instant::now());
            }

            // emulate
            let mut guard = self.state.state.lock();
            while self.state.advance.get() {
                // wait until the next slice should run
                while next > Instant::now() {
                    std::hint::spin_loop();
                    std::thread::yield_now();

                    // if we should stop, go to outer
                    if !self.state.advance.get() {
                        continue 'outer;
                    }
                }

                // emulate
                let locked = &mut *guard;
                let emulated = if locked.breakpoints.is_empty() {
                    let executed = locked.core.run(Limits::cycles(STEP_SIZE));
                    executed.cycles
                } else {
                    std::hint::cold_path();

                    let (executed, hit) = locked
                        .core
                        .run_breakpoints(Limits::cycles(STEP_SIZE), &locked.breakpoints);

                    if hit {
                        tracing::debug!("hit {}", locked.core.system.cpu.pc);
                        self.state.advance.set(false);
                        self.state.breakpoint.set(true);
                    }

                    executed.cycles
                };

                // calculate when the next slice should run
                next += to_duration(emulated);

                // avoid acumulating slowdowns
                next = next.max(Instant::now());
            }
        }
    }
}
