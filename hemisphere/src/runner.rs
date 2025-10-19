//! [`Runner`] abstracts over the emulator and lets you easily control it.

mod worker;

use crate::Hemisphere;
use common::Address;
use parking_lot::Mutex;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

struct RelaxedBool {
    value: AtomicBool,
}

impl RelaxedBool {
    fn new(value: bool) -> Self {
        Self {
            value: AtomicBool::new(value),
        }
    }

    #[inline(always)]
    fn get(&self) -> bool {
        self.value.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn set(&self, value: bool) {
        self.value.store(value, Ordering::Relaxed);
    }
}

pub struct State {
    core: Hemisphere,
    breakpoints: Vec<Address>,
}

impl State {
    pub fn core(&self) -> &Hemisphere {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut Hemisphere {
        &mut self.core
    }

    pub fn breakpoints(&self) -> &[Address] {
        &self.breakpoints
    }

    pub fn add_breakpoint(&mut self, addr: Address) {
        if !self.breakpoints.contains(&addr) {
            self.breakpoints.push(addr);
        }
    }

    pub fn remove_breakpoint(&mut self, addr: Address) {
        self.breakpoints.retain(|b| *b != addr);
    }

    pub fn toggle_breakpoint(&mut self, addr: Address) {
        if self.breakpoints.contains(&addr) {
            self.remove_breakpoint(addr);
        } else {
            self.add_breakpoint(addr);
        }
    }
}

/// Runner state shared with the worker thread.
struct Shared {
    state: Mutex<State>,
    advance: RelaxedBool,
    breakpoint: RelaxedBool,
}

/// A runner for the Hemisphere emulator.
pub struct Runner {
    state: Arc<Shared>,
    handle: JoinHandle<()>,
}

impl Runner {
    /// Creates a runner with a new emulator instance using the given config.
    pub fn new(config: crate::Config) -> Self {
        let core = Hemisphere::new(config);
        let state = Arc::new(Shared {
            state: Mutex::new(State {
                core,
                breakpoints: vec![],
            }),
            advance: RelaxedBool::new(false),
            breakpoint: RelaxedBool::new(false),
        });

        let worker = worker::WorkerThread::new(state.clone());
        let handle = std::thread::Builder::new()
            .name("hemi-runner".to_owned())
            .spawn(move || worker.run())
            .unwrap();

        Self { state, handle }
    }

    /// Whether the emulation thread is running.
    pub fn running(&self) -> bool {
        self.state.advance.get()
    }

    /// Continue or pause the emulation thread.
    pub fn set_run(&mut self, run: bool) {
        self.state.advance.set(run)
    }

    /// Single-step the emulator core.
    pub fn step(&mut self) {
        self.with_state(|e| e.core.step());
    }

    /// Pauses the emulation thread and executes a closure with the emulator core.
    pub fn with_state<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut State) -> R,
    {
        assert!(!self.handle.is_finished(), "runner thread died");

        let current = self.state.advance.get();
        self.state.advance.set(false);

        let mut state = self.state.state.lock();
        let result = f(&mut state);

        let breakpoint = self.state.breakpoint.get();
        if breakpoint {
            self.state.breakpoint.set(false);
        }

        if current && !breakpoint {
            self.state.advance.set(true);
        }

        result
    }
}
