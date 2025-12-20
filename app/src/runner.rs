use hemisphere::Hemisphere;
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};

struct State {
    hemi: Mutex<Hemisphere>,
    advance: AtomicBool,
}

fn worker(state: Arc<State>) {
    loop {
        if !state.advance.load(Ordering::Relaxed) {
            continue;
        }
    }
}

pub struct Runner {
    state: Arc<State>,
}

impl Runner {
    pub fn new(hemisphere: Hemisphere) -> Self {
        let state = State {
            hemi: Mutex::new(hemisphere),
            advance: AtomicBool::new(true),
        };

        let state = Arc::new(state);
        std::thread::spawn({
            let state = state.clone();
            move || worker(state)
        });

        Self { state }
    }

    pub fn start(&mut self) {
        self.state.advance.store(true, Ordering::Release);
    }

    pub fn stop(&mut self) {
        self.state.advance.store(false, Ordering::Relaxed);
    }

    pub fn get(&mut self) -> Option<MutexGuard<'_, Hemisphere>> {
        if self.state.advance.load(Ordering::Acquire) {
            return None;
        }

        Some(self.state.hemi.lock().unwrap())
    }
}
