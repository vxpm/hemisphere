use crate::system::System;
use std::collections::VecDeque;

pub struct ScheduledEvent {
    pub cycle: u64,
    pub handler: fn(&mut System),
}

pub struct Scheduler {
    elapsed: u64,
    scheduled: VecDeque<ScheduledEvent>,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler")
            .field("elapsed", &self.elapsed)
            .field("scheduled", &self.scheduled.len())
            .finish()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self {
            elapsed: 0,
            scheduled: VecDeque::with_capacity(16),
        }
    }
}

impl Scheduler {
    #[inline(always)]
    pub fn schedule(&mut self, after: u64, handler: fn(&mut System)) {
        let cycle = self.elapsed + after;
        let index = self.scheduled.partition_point(|e| e.cycle <= cycle);
        self.scheduled
            .insert(index, ScheduledEvent { cycle, handler });
    }

    #[inline(always)]
    pub fn schedule_now(&mut self, handler: fn(&mut System)) {
        self.schedule(0, handler)
    }

    #[inline(always)]
    pub fn cancel(&mut self, handler: fn(&mut System)) {
        self.scheduled
            .retain(|x| !std::ptr::fn_addr_eq(x.handler, handler));
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.scheduled.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub fn advance(&mut self, count: u64) {
        self.elapsed += count;
    }

    #[inline(always)]
    pub fn until_next(&self) -> Option<u64> {
        self.scheduled.front().map(|e| e.cycle - self.elapsed)
    }

    #[inline(always)]
    pub fn pop(&mut self) -> Option<fn(&mut System)> {
        self.scheduled
            .pop_front_if(|e| e.cycle <= self.elapsed)
            .map(|e| e.handler)
    }

    /// How many CPU cycles have elapsed.
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
