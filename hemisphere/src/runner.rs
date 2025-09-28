//! [`Runner`] abstracts over the emulator and lets you easily control it.

use crate::{Hemisphere, Limits};
use color_backtrace::BacktracePrinter;
use common::{Address, arch::FREQUENCY};
use parking_lot::FairMutex;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const STEP_SIZE: u32 = 8 * 1024;

#[inline(always)]
fn to_duration(cycles: u32) -> Duration {
    Duration::from_secs_f64(cycles as f64 / FREQUENCY as f64)
}

#[derive(Default)]
pub struct Stats {
    /// Cycles per second, for the last 1024 slices.
    pub cps: VecDeque<f32>,
}

/// Emulation state.
pub struct State {
    hemisphere: Hemisphere,
    breakpoints: Vec<Address>,
    stats: Stats,
}

impl State {
    pub fn new(hemisphere: Hemisphere) -> Self {
        Self {
            hemisphere,
            breakpoints: Vec::new(),
            stats: Stats::default(),
        }
    }

    pub fn hemisphere(&self) -> &Hemisphere {
        &self.hemisphere
    }

    pub fn hemisphere_mut(&mut self) -> &mut Hemisphere {
        &mut self.hemisphere
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    pub fn breakpoints(&self) -> &[Address] {
        &self.breakpoints
    }

    pub fn breakpoints_mut(&mut self) -> &mut Vec<Address> {
        &mut self.breakpoints
    }
}

struct Control {
    should_run: AtomicBool,
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

fn run(state: Arc<FairMutex<State>>, control: Arc<Control>) {
    setup_panic_hook();

    let mut next = Instant::now();
    'outer: loop {
        // sleep while shouldnt run
        if !control.should_run.load(Ordering::Relaxed) {
            while !control.should_run.load(Ordering::Relaxed) {
                std::thread::park();
            }

            next = next.max(Instant::now());
        }

        // wait until the next slice should run
        while next > Instant::now() {
            if !control.should_run.load(Ordering::Relaxed) {
                continue 'outer;
            }

            std::thread::yield_now();
        }

        // emulate
        let mut guard = state.lock();
        let state = &mut *guard;

        let emulated = if state.breakpoints.is_empty() {
            let executed = state.hemisphere.run(Limits::cycles(STEP_SIZE));
            executed.cycles
        } else {
            std::hint::cold_path();

            let (executed, hit) = state
                .hemisphere
                .run_breakpoints(Limits::cycles(STEP_SIZE), &state.breakpoints);

            if hit {
                control.should_run.store(false, Ordering::Relaxed);
            }

            executed.cycles
        };

        if guard.stats.cps.len() >= 1024 {
            guard.stats.cps.pop_back();
        }

        let cps = emulated as f32 / next.elapsed().as_secs_f32();
        guard.stats.cps.push_front(cps);

        // calculate when the next slice should run
        next += to_duration(emulated);

        // avoid acumulating slowdowns
        next = next.max(Instant::now());
    }
}

/// A runner for the Hemisphere emulator.
pub struct Runner {
    state: Arc<FairMutex<State>>,
    control: Arc<Control>,
    handle: JoinHandle<()>,
}

impl Runner {
    /// Creates a runner with a new emulator instance using the given config.
    pub fn new(config: crate::Config) -> Self {
        let hemisphere = Hemisphere::new(config);
        let state = Arc::new(FairMutex::new(State::new(hemisphere)));
        let control = Arc::new(Control {
            should_run: AtomicBool::new(false),
        });

        let handle = std::thread::Builder::new()
            .name("hemi-runner".to_owned())
            .spawn({
                let state = state.clone();
                let control = control.clone();

                || run(state, control)
            })
            .unwrap();

        Self {
            state,
            control,
            handle,
        }
    }

    /// Whether the emulation thread is running.
    pub fn running(&self) -> bool {
        self.control.should_run.load(Ordering::Relaxed)
    }

    /// Continue or pause the emulation thread.
    pub fn set_run(&mut self, run: bool) {
        self.control.should_run.store(run, Ordering::Relaxed);
        if run {
            self.handle.thread().unpark();
        }
    }

    /// Single-step the emulator if it isn't running.
    pub fn step(&mut self) {
        if !self.running() {
            self.with_state(|s| s.hemisphere.step());
        }
    }

    /// Pauses the emulation thread and executes a closure with the emulation state.
    pub fn with_state<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut State) -> R,
    {
        if self.handle.is_finished() {
            panic!("runner thread died");
        }

        let mut state = self.state.lock();
        f(&mut state)
    }
}
