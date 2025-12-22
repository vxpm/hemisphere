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
    pub cycles_history: VecDeque<(Cycles, Instant)>,
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

    let mut late = Duration::ZERO;
    let mut next = Instant::now();

    loop {
        sleeper.sleep_until(next);
        while !state.advance.load(Ordering::Relaxed) {
            sleeper.sleep(Duration::from_micros(1));
        }
        let start = Instant::now();

        let mut lock = state.state.lock().unwrap();
        let state = &mut *lock;

        let to_exec = (STEP + late).min(4 * STEP);
        let executed = state
            .emulator
            .exec(Cycles::from_duration(to_exec), &state.breakpoints);

        while let Some(front) = state.cycles_history.front()
            && start.duration_since(front.1) > Duration::from_secs(1)
        {
            state.cycles_history.pop_front();
        }
        state.cycles_history.push_back((executed.cycles, start));

        late = start.elapsed().saturating_sub(to_exec);
        next = (next + STEP).max(Instant::now());
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
                cycles_history: VecDeque::new(),
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
