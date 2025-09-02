use hemicore::Address;
use tracing::debug;

use crate::{FREQUENCY, Hemisphere};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const STEP_SIZE: u32 = 4096;

#[inline(always)]
fn to_duration(cycles: u32) -> Duration {
    Duration::from_secs_f64(cycles as f64 / FREQUENCY as f64)
}

#[derive(Default)]
pub struct Stats {
    /// Instructions per second, for the last 1024 slices.
    pub ips: VecDeque<f32>,
}

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

fn run(state: Arc<Mutex<State>>, control: Arc<Control>) {
    let mut next = Instant::now();
    let mut guard = state.lock().unwrap();
    'outer: loop {
        if !control.should_run.load(Ordering::Relaxed) {
            std::mem::drop(guard);

            while !control.should_run.load(Ordering::Relaxed) {
                std::thread::park();
            }

            guard = state.lock().unwrap();
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
        // NOTE: assume 2 cycles per instruction
        let mut emulated = 0;
        while emulated < STEP_SIZE {
            if guard.breakpoints.is_empty() {
                emulated += 2 * guard.hemisphere.exec();
                continue;
            } else {
                std::hint::cold_path();
            }

            let mut target = Address(0);
            let mut min_distance = u32::MAX;
            for breakpoint in guard.breakpoints.iter().copied() {
                let distance = breakpoint
                    .value()
                    .checked_sub(guard.hemisphere.system.cpu.pc.value());

                if let Some(distance) = distance
                    && distance <= min_distance
                    && distance != 0
                {
                    target = breakpoint;
                    min_distance = distance;
                }
            }

            let target_distance = min_distance / 4;
            if target_distance <= guard.hemisphere.config.instructions_per_block as u32 {
                emulated += 2 * guard.hemisphere.exec_limited(target_distance as u16);
                if guard.hemisphere.system.cpu.pc == target {
                    control.should_run.store(false, Ordering::Relaxed);
                    break;
                }
            } else {
                emulated += 2 * guard.hemisphere.exec();
            }
        }

        if guard.stats.ips.len() >= 1024 {
            guard.stats.ips.pop_back();
        }

        guard
            .stats
            .ips
            .push_front(emulated as f32 / next.elapsed().as_secs_f32());

        // calculate when the next slice should run
        next += to_duration(emulated);

        // avoid acumulating slowdowns
        next = next.max(Instant::now());
    }
}

/// A simple runner for the Hemisphere emulator.
pub struct Runner {
    state: Arc<Mutex<State>>,
    control: Arc<Control>,
    handle: JoinHandle<()>,
}

impl Runner {
    pub fn new(hemisphere: Hemisphere) -> Self {
        let state = Arc::new(Mutex::new(State::new(hemisphere)));
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

    pub fn running(&self) -> bool {
        self.control.should_run.load(Ordering::Relaxed)
    }

    pub fn set_run(&mut self, run: bool) {
        self.control.should_run.store(run, Ordering::Relaxed);
        if run {
            self.handle.thread().unpark();
        }
    }

    pub fn with_state<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut State) -> R,
    {
        let run = self.control.should_run.load(Ordering::Relaxed);
        self.control.should_run.store(false, Ordering::Relaxed);

        let mut state = self.state.lock().unwrap();
        let result = f(&mut state);

        if run {
            self.control.should_run.store(true, Ordering::Relaxed);
            self.handle.thread().unpark();
        }

        result
    }
}
