use hemisphere::{
    Config, Hemisphere,
    dolfile::{Dol, binrw::BinRead},
};
use std::{
    io::BufReader,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self},
};

pub struct State {
    pub emulator: Hemisphere,
}

struct Shared {
    state: Mutex<State>,
    requested: AtomicBool,
}

pub struct Emulator {
    shared: Arc<Shared>,
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
            requested: AtomicBool::new(false),
        });

        thread::Builder::new()
            .name("hemisphere-emu".into())
            .spawn({
                let shared = shared.clone();
                move || emulator_thread(shared)
            })
            .unwrap();

        Self { shared }
    }

    pub fn state(&self) -> MutexGuard<'_, State> {
        self.shared.requested.store(true, Ordering::Relaxed);
        let state = self.shared.state.lock().unwrap();
        self.shared.requested.store(false, Ordering::Relaxed);

        state
    }
}

fn emulator_thread(shared: Arc<Shared>) {
    loop {
        let mut state = shared.state.lock().unwrap();
        while !shared.requested.load(Ordering::Relaxed) {
            state.emulator.exec();
        }

        std::mem::drop(state);
        while shared.requested.load(Ordering::Relaxed) {}
    }
}
