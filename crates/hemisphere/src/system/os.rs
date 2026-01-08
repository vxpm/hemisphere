//! Dolphin-OS

use crate::system::System;
use bitos::{BitUtils, TryBits, bitos, integer::u4};
use gekko::Address;

#[derive(Debug, Clone)]
pub struct Context {
    pub sp: Address,
    pub srr0: Address,
}

#[bitos(4)]
#[derive(Debug, Clone, Copy, Default)]
pub enum State {
    #[default]
    Dead = 0b0000,
    Ready = 0b0001,
    Running = 0b0010,
    Waiting = 0b0100,
    Moribund = 0b1000,
}

#[derive(Debug, Clone, Copy)]
pub struct ThreadQueue {
    pub ptr_head: Address,
    pub ptr_tail: Address,
}

#[derive(Debug, Clone)]
pub struct ThreadData {
    pub context: Context,
    pub state: State,
    pub detached: bool,
    pub suspended: bool,
    pub priority: i32,
    pub base_priority: i32,
    pub ptr_parent_queue: Address,
    pub ptr_next: Address,
    pub ptr_prev: Address,
    pub ptr_active_next: Address,
    pub ptr_active_prev: Address,
    pub ptr_stack_base: Address,
    pub ptr_stack_end: Address,
    pub error: i32,
}

#[derive(Debug, Clone)]
pub struct Thread {
    pub addr: Address,
    pub data: ThreadData,
}

pub fn thread(sys: &System, addr: Address) -> Option<Thread> {
    let sp = Address(sys.read_pure::<u32>(addr + 0x4)?);
    let srr0 = Address(sys.read_pure::<u32>(addr + 0x198)?);
    let state = State::try_from_bits(u4::new(sys.read_pure::<u16>(addr + 0x2C8)? as u8))
        .unwrap_or_default();
    let detached = sys.read_pure::<u32>(addr + 0x2CA)?.bit(0);
    let suspended = sys.read_pure::<u32>(addr + 0x2CC)? > 0;
    let priority = sys.read_pure::<i32>(addr + 0x2D0)?;
    let base_priority = sys.read_pure::<i32>(addr + 0x2D4)?;
    let ptr_parent_queue = Address(sys.read_pure::<u32>(addr + 0x2DC)?);
    let ptr_next = Address(sys.read_pure::<u32>(addr + 0x2E0)?);
    let ptr_prev = Address(sys.read_pure::<u32>(addr + 0x2E4)?);
    let ptr_active_next = Address(sys.read_pure::<u32>(addr + 0x2FC)?);
    let ptr_active_prev = Address(sys.read_pure::<u32>(addr + 0x300)?);
    let ptr_stack_base = Address(sys.read_pure::<u32>(addr + 0x304)?);
    let ptr_stack_end = Address(sys.read_pure::<u32>(addr + 0x308)?);
    let error = sys.read_pure::<i32>(addr + 0x30C)?;

    let data = ThreadData {
        context: Context { sp, srr0 },
        state,
        detached,
        suspended,
        priority,
        base_priority,
        ptr_parent_queue,
        ptr_next,
        ptr_prev,
        ptr_active_next,
        ptr_active_prev,
        ptr_stack_base,
        ptr_stack_end,
        error,
    };

    Some(Thread { addr, data })
}

#[derive(Debug, Clone)]
pub struct Threads {
    pub default: Thread,
    pub current: Option<Thread>,
    pub active: Vec<Thread>,
}

pub fn active_threads(sys: &System) -> Option<Vec<Thread>> {
    let mut threads = vec![];
    let mut current = Address(sys.read_pure::<u32>(Address(0x8000_00DC))?);
    while !current.is_null() {
        let Some(thread) = thread(sys, current) else {
            break;
        };

        current = thread.data.ptr_active_next;
        threads.push(thread);
    }

    Some(threads)
}

pub fn system_threads(sys: &System) -> Option<Threads> {
    let default = thread(sys, Address(sys.read_pure::<u32>(Address(0x8000_00DC))?))?;

    let ptr_current = Address(sys.read_pure::<u32>(Address(0x8000_00E4))?);
    let current = (!ptr_current.is_null())
        .then(|| thread(sys, ptr_current))
        .flatten();

    let active = active_threads(sys)?;

    Some(Threads {
        default,
        current,
        active,
    })
}
