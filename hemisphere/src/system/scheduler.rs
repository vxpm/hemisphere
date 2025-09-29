use crate::system::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledEvent {
    pub cycle: u64,
    pub event: Event,
}

#[derive(Debug)]
pub struct Scheduler {
    elapsed: u64,
    scheduled: Vec<ScheduledEvent>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self {
            elapsed: 0,
            scheduled: Vec::with_capacity(16),
        }
    }
}

impl Scheduler {
    #[inline(always)]
    pub fn schedule(&mut self, event: Event, after: u64) {
        self.scheduled.push(ScheduledEvent {
            cycle: self.elapsed + after,
            event,
        });

        self.scheduled.sort_unstable_by_key(|e| e.cycle);
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.scheduled.len()
    }

    #[inline(always)]
    pub fn advance(&mut self, count: u64) {
        self.elapsed += count;
    }

    #[inline(always)]
    pub fn until_next(&self) -> Option<u64> {
        self.scheduled.first().map(|e| e.cycle - self.elapsed)
    }

    #[inline(always)]
    pub fn pop(&mut self) -> Option<Event> {
        self.scheduled
            .iter()
            .position(|e| e.cycle <= self.elapsed)
            .map(|i| self.scheduled.swap_remove(i).event)
    }

    #[inline(always)]
    pub fn retain(&mut self, f: impl FnMut(&ScheduledEvent) -> bool) {
        self.scheduled.retain(f);
    }

    #[inline(always)]
    pub fn elapsed(&self) -> u64 {
        self.elapsed
    }

    /// How many time base cycles have elapsed.
    #[inline(always)]
    pub fn elapsed_time_base(&self) -> u64 {
        self.elapsed / 12
    }
}
