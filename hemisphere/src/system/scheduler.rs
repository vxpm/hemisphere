use crate::system::System;
use smallbox::{SmallBox, smallbox, space::S4};
use std::{
    any::{Any, TypeId},
    ops::{Deref, DerefMut},
};

pub trait EventHandler: FnMut(&mut System) + 'static {}
impl<T> EventHandler for T where T: FnMut(&mut System) + 'static {}

pub struct ErasedEventHandler(SmallBox<dyn EventHandler, S4>);

impl Deref for ErasedEventHandler {
    type Target = dyn FnMut(&mut System);

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl DerefMut for ErasedEventHandler {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.0
    }
}

pub struct ScheduledEvent {
    pub tag: TypeId,
    pub cycle: u64,
    pub handler: ErasedEventHandler,
}

impl PartialEq for ScheduledEvent {
    fn eq(&self, other: &Self) -> bool {
        self.tag == other.tag
    }
}

pub struct Scheduler {
    elapsed: u64,
    scheduled: Vec<ScheduledEvent>,
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
            scheduled: Vec::with_capacity(16),
        }
    }
}

struct Untagged;

impl Scheduler {
    pub fn schedule_tagged<Tag>(&mut self, after: u64, handler: impl EventHandler)
    where
        Tag: Any,
    {
        self.scheduled.push(ScheduledEvent {
            tag: TypeId::of::<Tag>(),
            cycle: self.elapsed + after,
            handler: ErasedEventHandler(smallbox!(handler)),
        });

        self.scheduled.sort_unstable_by_key(|e| e.cycle);
    }

    #[inline(always)]
    pub fn schedule<E>(&mut self, after: u64, handler: E)
    where
        E: EventHandler,
    {
        self.schedule_tagged::<Untagged>(after, handler);
    }

    #[inline(always)]
    pub fn schedule_now<E>(&mut self, handler: E)
    where
        E: EventHandler,
    {
        self.schedule(0, handler)
    }

    #[inline(always)]
    pub fn cancel<T>(&mut self)
    where
        T: Any,
    {
        let tag = TypeId::of::<T>();
        self.scheduled.retain(|x| x.tag != tag);
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
        self.scheduled.first().map(|e| e.cycle - self.elapsed)
    }

    #[inline(always)]
    pub fn pop(&mut self) -> Option<ErasedEventHandler> {
        self.scheduled
            .iter()
            .position(|e| e.cycle <= self.elapsed)
            .map(|i| self.scheduled.swap_remove(i).handler)
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
