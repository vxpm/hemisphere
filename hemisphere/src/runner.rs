use crate::{FREQUENCY, Hemisphere};
use std::{
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const STEP_SIZE: u32 = 4096;

fn to_duration(cycles: u32) -> Duration {
    Duration::from_secs_f64(cycles as f64 / FREQUENCY as f64)
}

pub struct State {
    pub hemisphere: Hemisphere,
}

struct Control {
    should_run: AtomicBool,
}

fn run(state: Arc<Mutex<State>>, control: Arc<Control>) {
    let mut next = Instant::now();

    loop {
        while !control.should_run.load(Ordering::Relaxed) {
            std::thread::park();
            next = Instant::now();
        }

        // wait until the next slice should run
        while next < Instant::now() {
            std::hint::spin_loop();
        }

        // emulate
        let mut state = state.lock().unwrap();
        let mut emulated = 0;
        while emulated < STEP_SIZE {
            emulated += state.hemisphere.exec();
        }

        // calculate when the next slice should run
        let extra = emulated - STEP_SIZE;
        next += to_duration(STEP_SIZE + extra);

        // avoid acumulating slowdowns
        next = next.max(Instant::now());
    }
}

pub struct Runner {
    state: Arc<Mutex<State>>,
    control: Arc<Control>,
    handle: JoinHandle<()>,
}

impl Runner {
    pub fn new(hemisphere: Hemisphere) -> Self {
        let state = Arc::new(Mutex::new(State { hemisphere }));
        let control = Arc::new(Control {
            should_run: AtomicBool::new(false),
        });

        let handle = std::thread::spawn({
            let state = state.clone();
            let control = control.clone();

            || run(state, control)
        });

        Self {
            state,
            control,
            handle,
        }
    }

    pub fn set_run(&mut self, run: bool) {
        self.control.should_run.store(run, Ordering::Relaxed);
        if run {
            self.handle.thread().unpark();
        }
    }

    pub fn state(&mut self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }
}
