use hemisphere::{
    Config, Hemisphere,
    dolfile::{Dol, binrw::BinRead},
};
use std::{
    io::BufReader,
    ops::{Deref, DerefMut},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, Thread},
    time::{Duration, Instant},
};

pub struct State {
    pub emulator: Hemisphere,
}

struct Shared {
    state: Mutex<State>,
    advance: AtomicBool,
}

pub struct StateGuard<'a> {
    mutex: MutexGuard<'a, State>,
    thread: Thread,
    old_advance: bool,
    advance: &'a AtomicBool,
}

impl<'a> Deref for StateGuard<'a> {
    type Target = State;

    fn deref(&self) -> &Self::Target {
        &*self.mutex
    }
}

impl<'a> DerefMut for StateGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.mutex
    }
}

impl<'a> Drop for StateGuard<'a> {
    fn drop(&mut self) {
        self.advance.store(self.old_advance, Ordering::Release);
        self.thread.unpark();
    }
}

pub struct Emulator {
    shared: Arc<Shared>,
    thread: Thread,
}

impl Emulator {
    pub fn new() -> Self {
        let mut hemisphere = Hemisphere::new(Config {
            instructions_per_block: 64,
        });

        let dol = Dol::read(&mut BufReader::new(
            std::fs::File::open("panda.dol").unwrap(),
        ))
        .unwrap();
        hemisphere.load(&dol);

        let state = Mutex::new(State {
            emulator: hemisphere,
        });

        let shared = Arc::new(Shared {
            state,
            advance: AtomicBool::new(false),
        });

        let handle = thread::Builder::new()
            .name("hemisphere-emu".into())
            .spawn({
                let shared = shared.clone();
                move || emulator_thread(shared)
            })
            .unwrap();

        Self {
            shared,
            thread: handle.thread().clone(),
        }
    }

    pub fn state(&self) -> StateGuard<'_> {
        let old_advance = self.shared.advance.load(Ordering::Relaxed);

        self.shared.advance.store(false, Ordering::Relaxed);
        let state = self.shared.state.lock().unwrap();

        StateGuard {
            mutex: state,
            old_advance,
            advance: &self.shared.advance,
            thread: self.thread.clone(),
        }
    }

    pub fn running(&self) -> bool {
        self.shared.advance.load(Ordering::Relaxed)
    }

    pub fn run(&mut self) {
        self.shared.advance.store(true, Ordering::Relaxed);
    }

    pub fn stop(&mut self) {
        self.shared.advance.store(false, Ordering::Relaxed);
    }
}

const FREQUENCY: u32 = 486_000_000;
const CYCLES_PER_SLICE: u32 = FREQUENCY / 200_000;

#[derive(Default)]
struct MainLoop {}

impl MainLoop {
    fn emulate(&mut self, state: &mut State) {
        let slice_duration = Duration::from_secs_f64(CYCLES_PER_SLICE as f64 / FREQUENCY as f64);
        let begin = Instant::now();

        let mut emulated = 0;
        while emulated < CYCLES_PER_SLICE {
            // assume an average of 2 cycles per instruction
            emulated += state.emulator.exec() * 2;
        }

        while begin.elapsed() < slice_duration {}
    }
}

fn emulator_thread(shared: Arc<Shared>) {
    let mut main_loop = MainLoop::default();
    loop {
        while !shared.advance.load(Ordering::Acquire) {
            thread::park();
        }

        let mut state = shared.state.lock().unwrap();
        while shared.advance.load(Ordering::Relaxed) {
            main_loop.emulate(&mut state);
        }
    }
}
