mod timer;

use lazuli::{Address, Cycles, Lazuli};
use spin_sleep::SpinSleeper;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use crate::runner::timer::Timer;

pub struct State {
    pub hemi: Lazuli,
    pub breakpoints: Vec<Address>,
    pub cycles_history: VecDeque<(Cycles, Duration)>,
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

fn worker(runner_state: Arc<Shared>) {
    let sleeper = SpinSleeper::default();

    let mut timer = Timer::new();
    let mut emulated = Duration::ZERO;

    loop {
        if runner_state.advance.load(Ordering::Relaxed) {
            timer.resume();
        } else {
            timer.pause();

            // TODO: properly deal with this
            std::thread::yield_now();
            continue;
        }

        // compute how far behind real-time we are
        let delta = timer.elapsed().saturating_sub(emulated);

        // wait until delta >= STEP
        let to_sleep = STEP.saturating_sub(delta);
        if !to_sleep.is_zero() {
            sleeper.sleep(to_sleep);
        }

        let now = timer.elapsed();

        // ignore slowdowns that are too large (~1 frame at 60fps)
        let delta = if delta > Duration::from_millis(16) {
            emulated = now - STEP;
            STEP
        } else {
            now.saturating_sub(emulated)
        };

        let mut lock = runner_state.state.lock().unwrap();
        let state = &mut *lock;

        let executed = state
            .hemi
            .exec(Cycles::from_duration(delta), &state.breakpoints);

        emulated += delta;

        if executed.hit_breakpoint {
            runner_state.advance.store(false, Ordering::SeqCst);
        }

        while let Some(front) = state.cycles_history.front()
            && now.saturating_sub(front.1) > Duration::from_millis(500)
        {
            state.cycles_history.pop_front();
        }
        state.cycles_history.push_back((executed.cycles, now));
    }
}

pub struct Runner {
    shared: Arc<Shared>,
}

impl Runner {
    pub fn new(lazuli: Lazuli) -> Self {
        let state = Shared {
            state: Mutex::new(State {
                hemi: lazuli,
                breakpoints: vec![],
                cycles_history: VecDeque::new(),
            }),
            advance: AtomicBool::new(false),
        };

        let state = Arc::new(state);
        std::thread::Builder::new()
            .name("lazuli runner".into())
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

    pub fn step(&mut self) {
        if !self.running() {
            let mut lock = self.shared.state.lock().unwrap();
            lock.hemi.step();
        }
    }

    pub fn running(&mut self) -> bool {
        self.shared.advance.load(Ordering::Relaxed)
    }

    pub fn get(&mut self) -> MutexGuard<'_, State> {
        self.shared.state.lock().unwrap()
    }
}
