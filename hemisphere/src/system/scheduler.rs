use crate::system::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScheduledEvent {
    cycle: u64,
    event: Event,
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
            scheduled: vec![ScheduledEvent {
                cycle: 0,
                event: Event::Decrementer,
            }],
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
    pub fn elapsed(&self) -> u64 {
        self.elapsed
    }
}
