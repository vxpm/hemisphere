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
        let mut hemisphere = Hemisphere::new(Config::default());
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

fn emulator_thread(shared: Arc<Shared>) {
    loop {
        while !shared.advance.load(Ordering::Acquire) {
            thread::park();
        }

        let mut state = shared.state.lock().unwrap();
        while shared.advance.load(Ordering::Relaxed) {
            state.emulator.exec();
        }
    }
}
