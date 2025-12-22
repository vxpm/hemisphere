use std::time::{Duration, Instant};

pub struct Timer {
    elapsed: Duration,
    resumed_at: Instant,
    running: bool,
    scale: f64,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            elapsed: Duration::ZERO,
            resumed_at: Instant::now(),
            running: false,
            scale: 1.0,
        }
    }

    #[inline(always)]
    pub fn scale(&self) -> f64 {
        self.scale
    }

    #[inline(always)]
    pub fn set_scale(&mut self, value: f64) {
        if self.running {
            self.pause();
            self.scale = value;
            self.resume();
        } else {
            self.scale = value;
        }
    }

    #[inline(always)]
    pub fn resume(&mut self) {
        if !self.running {
            self.resumed_at = Instant::now();
            self.running = true;
        }
    }

    #[inline(always)]
    pub fn pause(&mut self) {
        if self.running {
            self.elapsed += self.resumed_at.elapsed().mul_f64(self.scale);
            self.running = false;
        }
    }

    #[inline(always)]
    pub fn elapsed(&self) -> Duration {
        if self.running {
            self.elapsed + self.resumed_at.elapsed().mul_f64(self.scale)
        } else {
            self.elapsed
        }
    }
}
