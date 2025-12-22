use hemisphere::{Address, Cycles, Hemisphere};
use spin_sleep::SpinSleeper;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub struct State {
    pub emulator: Hemisphere,
    pub breakpoints: Vec<Address>,
    pub cps_history: VecDeque<f64>,
}

impl State {
    pub fn add_breakpoint(&mut self, breakpoint: Address) {
        if !self.breakpoints.contains(&breakpoint) {
            self.breakpoints.push(breakpoint);
        }
    }

    pub fn remove_breakpoint(&mut self, breakpoint: Address) {
        self.breakpoints.retain(|b| *b != breakpoint);
    }
}

struct Shared {
    state: Mutex<State>,
    advance: AtomicBool,
}

const STEP: Duration = Duration::from_millis(1);

fn worker(state: Arc<Shared>) {
    let sleeper = SpinSleeper::default();

    let mut prev = Instant::now();
    loop {
        sleeper.sleep_until(prev + STEP);
        while !state.advance.load(Ordering::Relaxed) {
            prev = Instant::now() - STEP;
        }

        let start = Instant::now();
        let mut lock = state.state.lock().unwrap();
        let state = &mut *lock;

        let executed = state
            .emulator
            .exec(Cycles::from_duration(prev.elapsed()), &state.breakpoints);

        if state.cps_history.len() >= 256 {
            state.cps_history.pop_front();
        }

        state
            .cps_history
            .push_back(executed.cycles.value() as f64 / start.elapsed().as_secs_f64());

        prev = start;
    }
}

pub struct Runner {
    shared: Arc<Shared>,
}

impl Runner {
    pub fn new(hemisphere: Hemisphere) -> Self {
        let state = Shared {
            state: Mutex::new(State {
                emulator: hemisphere,
                breakpoints: vec![],
                cps_history: VecDeque::new(),
            }),
            advance: AtomicBool::new(true),
        };

        let state = Arc::new(state);
        std::thread::Builder::new()
            .name("hemisphere runner".into())
            .spawn({
                let state = state.clone();
                move || worker(state)
            })
            .unwrap();

        Self { shared: state }
    }

    pub fn start(&mut self) {
        self.shared.advance.store(true, Ordering::Release);
    }

    pub fn stop(&mut self) {
        self.shared.advance.store(false, Ordering::Relaxed);
    }

    pub fn running(&mut self) -> bool {
        self.shared.advance.load(Ordering::Relaxed)
    }

    pub fn get(&mut self) -> Option<MutexGuard<'_, State>> {
        Some(self.shared.state.lock().unwrap())
    }
}
