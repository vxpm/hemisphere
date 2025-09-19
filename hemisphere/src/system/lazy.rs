use crate::system::System;

#[derive(Debug, Default)]
pub struct Lazy {
    pub last_updated_tb: u64,
    pub last_updated_dec: u64,
}

impl System {
    pub fn update_time_base(&mut self) {
        let last_updated = self.lazy.last_updated_tb;
        let now = self.scheduler.elapsed();
        let delta = now - last_updated;

        let prev = self.cpu.supervisor.misc.tb;
        let new = prev.wrapping_add(delta);

        tracing::debug!(
            "updating time base - now {now}, last updated {last_updated}, since then {delta}. prev: {prev}, new: {new}"
        );

        self.lazy.last_updated_tb = now;
        self.cpu.supervisor.misc.tb = new;
    }

    pub fn update_decrementer(&mut self) {
        let last_updated = self.lazy.last_updated_dec;
        let now = self.scheduler.elapsed();
        let delta = now - last_updated;

        let prev = self.cpu.supervisor.misc.dec;
        let new = prev.wrapping_sub(delta as u32);

        tracing::debug!(
            "updating dec - now {now}, last updated {last_updated}, since then {delta}. prev: {prev}, new: {new}"
        );

        self.lazy.last_updated_dec = now;
        self.cpu.supervisor.misc.dec = new;
    }
}
