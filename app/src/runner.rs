mod timer;

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

use crate::runner::timer::Timer;

pub struct State {
    pub emulator: Hemisphere,
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

fn worker(state: Arc<Shared>) {
    let sleeper = SpinSleeper::default();

    let mut timer = Timer::new();
    let mut emulated = Duration::ZERO;

    loop {
        if state.advance.load(Ordering::Relaxed) {
            timer.resume();
        } else {
            timer.pause();
            sleeper.sleep(Duration::from_micros(1));
            continue;
        }

        // compute how far behind real-time we are
        let delta = timer.elapsed().saturating_sub(emulated);

        // wait until delta >= STEP
        sleeper.sleep(STEP.saturating_sub(delta));
        let now = timer.elapsed();

        // ignore slowdowns that are too large (~1 frame at 60fps)
        let delta = if delta > Duration::from_millis(16) {
            emulated = now - STEP;
            STEP
        } else {
            now.saturating_sub(emulated)
        };

        let mut lock = state.state.lock().unwrap();
        let state = &mut *lock;

        let executed = state
            .emulator
            .exec(Cycles::from_duration(delta), &state.breakpoints);

        emulated += delta;

        while let Some(front) = state.cycles_history.front()
            && now.saturating_sub(front.1) > Duration::from_secs(1)
        {
            state.cycles_history.pop_front();
        }
        state.cycles_history.push_back((executed.cycles, now));
    }

    // let mut late = Duration::ZERO;
    // let mut next = Instant::now();
    // loop {
    // sleeper.sleep_until(next);
    // while !state.advance.load(Ordering::Relaxed) {
    //     sleeper.sleep(Duration::from_micros(1));
    // }
    // let start = Instant::now();
    //
    // let mut lock = state.state.lock().unwrap();
    // let state = &mut *lock;
    //
    // let to_exec = (STEP + late).min(4 * STEP);
    // let executed = state
    //     .emulator
    //     .exec(Cycles::from_duration(to_exec), &state.breakpoints);
    //
    // while let Some(front) = state.cycles_history.front()
    //     && start.duration_since(front.1) > Duration::from_secs(1)
    // {
    //     state.cycles_history.pop_front();
    // }
    // state.cycles_history.push_back((executed.cycles, start));
    //
    // late = start.elapsed().saturating_sub(to_exec);
    // next = (next + STEP).max(Instant::now());
    // }
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

    pub fn step(&mut self) {
        if !self.running() {
            let mut lock = self.shared.state.lock().unwrap();
            lock.emulator.step();
        }
    }

    pub fn running(&mut self) -> bool {
        self.shared.advance.load(Ordering::Relaxed)
    }

    pub fn get(&mut self) -> Option<MutexGuard<'_, State>> {
        Some(self.shared.state.lock().unwrap())
    }
}
