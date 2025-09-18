use crate::{System, system::Event};
use common::{Address, Primitive, arch::Cpu, util::boxed_array};
use interavl::IntervalTree;
use ppcjit::{
    Block,
    block::{GenericHook, GetRegistersHook, Hooks, ReadHook, WriteHook},
};
use slotmap::{SlotMap, new_key_type};
use std::{collections::BTreeMap, ops::Range};
use tracing::info;

const PAGE_COUNT: usize = 1 << 20;
type PageLUT = Box<[u16; PAGE_COUNT]>;

new_key_type! {
    pub struct BlockId;
}

pub struct BlockMapping {
    address: BTreeMap<Address, BlockId>,
    intervals: IntervalTree<Address, BlockId>,
    page_lut: PageLUT,

    // caching stuff
    buffer: Vec<Range<Address>>,
    last_query: Option<(Address, BlockId)>,
}

impl BlockMapping {
    fn insert(&mut self, range: Range<Address>, id: BlockId) {
        self.address.insert(range.start, id);
        self.intervals.insert(range.clone(), id);

        // update LUT
        let start_page = range.start.value() >> 12;
        let end_page = range.end.value() >> 12;
        for index in start_page..=end_page {
            self.page_lut[index as usize] += 1;
        }
    }

    /// Returns the block starting at `addr`.
    #[inline(always)]
    pub fn get(&mut self, addr: Address) -> Option<BlockId> {
        if let Some((last_addr, id)) = self.last_query
            && last_addr == addr
        {
            std::hint::cold_path();
            Some(id)
        } else {
            let id = *self.address.get(&addr)?;
            self.last_query = Some((addr, id));
            Some(id)
        }
    }

    /// Invalidates all blocks that contain `addr`.
    #[inline(always)]
    pub fn invalidate(&mut self, addr: Address) {
        // check LUT first
        let page = addr.value() >> 12;
        if self.page_lut[page as usize] == 0 {
            return;
        } else {
            std::hint::cold_path();
        }

        let range = addr..addr + 1;
        for (range, _) in self.intervals.iter_overlaps(&range) {
            self.buffer.push(range.clone());
        }

        for range in self.buffer.drain(..) {
            self.address.remove(&range.start);
            self.intervals.remove(&range);

            // update LUT
            let start_page = range.start.value() >> 12;
            let end_page = range.end.value() >> 12;
            for index in start_page..=end_page {
                self.page_lut[index as usize] -= 1;
            }
        }

        if self
            .last_query
            .as_ref()
            .is_some_and(|(queried, _)| *queried == addr)
        {
            self.last_query = None;
        }
    }

    pub fn clear(&mut self) {
        self.address.clear();
        self.intervals = IntervalTree::default();
        self.page_lut.fill(0);

        self.buffer.clear();
        self.last_query = None;
    }
}

impl Default for BlockMapping {
    fn default() -> Self {
        Self {
            address: BTreeMap::default(),
            intervals: IntervalTree::default(),
            page_lut: boxed_array(0),

            buffer: Vec::with_capacity(64),
            last_query: None,
        }
    }
}

/// A structure which keeps tracks of compiled [`Block`]s.
pub struct Blocks {
    pub storage: SlotMap<BlockId, Block>,
    pub mapping: BlockMapping,
}

impl Default for Blocks {
    fn default() -> Self {
        Self {
            storage: SlotMap::with_key(),
            mapping: BlockMapping::default(),
        }
    }
}

impl Blocks {
    #[inline(always)]
    pub fn insert(&mut self, addr: Address, block: Block) -> BlockId {
        let end = addr + block.meta().seq.len() as u32 * 4;
        let range = addr..end;

        let id = self.storage.insert(block);
        self.mapping.insert(range, id);

        id
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.storage.clear();
        self.mapping.clear();
    }
}

/// Context to be passed in for execution of JIT blocks.
pub struct Context<'a> {
    pub system: &'a mut System,
    pub mapping: &'a mut BlockMapping,
}

pub static CTX_HOOKS: Hooks = {
    extern "sysv64-unwind" fn get_registers<'a>(ctx: &'a mut Context) -> &'a mut Cpu {
        &mut ctx.system.cpu
    }

    extern "sysv64-unwind" fn read<T: Primitive>(ctx: &mut Context, addr: Address) -> T {
        // tracing::debug!(
        //     "pc: {}, r3 is {:08X}, r8 is {:08X}",
        //     ctx.system.cpu.pc,
        //     ctx.system.cpu.user.gpr[3],
        //     ctx.system.cpu.user.gpr[8],
        // );
        let physical = ctx.system.translate_data_addr(addr);
        ctx.system.bus.read(physical)
    }

    extern "sysv64-unwind" fn write<T: Primitive>(ctx: &mut Context, addr: Address, value: T) {
        let physical = ctx.system.translate_data_addr(addr);
        ctx.system.bus.write(physical, value);
        for i in 0..size_of::<T>() {
            ctx.mapping.invalidate(addr + i as u32);
        }
    }

    extern "sysv64-unwind" fn ibat_changed(ctx: &mut Context) {
        info!("ibats changed - clearing blocks mapping and rebuilding ibat lut");
        ctx.mapping.clear();
        ctx.system
            .mmu
            .build_instr_bat_lut(&ctx.system.cpu.supervisor.memory.ibat);
    }

    extern "sysv64-unwind" fn dbat_changed(ctx: &mut Context) {
        info!("dbats changed - rebuilding dbat lut");
        ctx.system
            .mmu
            .build_data_bat_lut(&ctx.system.cpu.supervisor.memory.dbat);
    }

    extern "sysv64-unwind" fn dec_read(ctx: &mut Context) {
        let last_updated = ctx.system.lazy.last_updated_dec;
        let elapsed = ctx.system.scheduler.elapsed();

        ctx.system.lazy.last_updated_dec = elapsed;
        ctx.system.cpu.supervisor.misc.dec = ctx
            .system
            .cpu
            .supervisor
            .misc
            .dec
            .wrapping_sub((elapsed - last_updated) as u32);
    }

    extern "sysv64-unwind" fn dec_changed(ctx: &mut Context) {
        ctx.system
            .scheduler
            .retain(|e| e.event != Event::Decrementer);

        let dec = ctx.system.cpu.supervisor.misc.dec;
        info!("decrementer changed to {dec}");

        ctx.system
            .scheduler
            .schedule(Event::Decrementer, dec as u64);
    }

    #[expect(
        clippy::missing_transmute_annotations,
        reason = "unnecessary - the definitions are above"
    )]
    unsafe {
        use std::mem::transmute;

        let get_registers =
            transmute::<_, GetRegistersHook>(get_registers as extern "sysv64-unwind" fn(_) -> _);

        let read_i8 =
            transmute::<_, ReadHook<i8>>(read::<i8> as extern "sysv64-unwind" fn(_, _) -> _);
        let write_i8 =
            transmute::<_, WriteHook<i8>>(write::<i8> as extern "sysv64-unwind" fn(_, _, _));
        let read_i16 =
            transmute::<_, ReadHook<i16>>(read::<i16> as extern "sysv64-unwind" fn(_, _) -> _);
        let write_i16 =
            transmute::<_, WriteHook<i16>>(write::<i16> as extern "sysv64-unwind" fn(_, _, _));
        let read_i32 =
            transmute::<_, ReadHook<i32>>(read::<i32> as extern "sysv64-unwind" fn(_, _) -> _);
        let write_i32 =
            transmute::<_, WriteHook<i32>>(write::<i32> as extern "sysv64-unwind" fn(_, _, _));

        let ibat_changed =
            transmute::<_, GenericHook>(ibat_changed as extern "sysv64-unwind" fn(_));
        let dbat_changed =
            transmute::<_, GenericHook>(dbat_changed as extern "sysv64-unwind" fn(_));

        let dec_read = transmute::<_, GenericHook>(dec_read as extern "sysv64-unwind" fn(_));
        let dec_changed = transmute::<_, GenericHook>(dec_changed as extern "sysv64-unwind" fn(_));

        Hooks {
            get_registers,

            read_i8,
            write_i8,
            read_i16,
            write_i16,
            read_i32,
            write_i32,

            ibat_changed,
            dbat_changed,

            dec_read,
            dec_changed,
        }
    }
};

/// The JIT context.
pub struct JIT {
    pub compiler: ppcjit::Compiler,
    pub blocks: Blocks,
}

impl Default for JIT {
    fn default() -> Self {
        Self::new()
    }
}

impl JIT {
    pub fn new() -> Self {
        Self {
            compiler: ppcjit::Compiler::default(),
            blocks: Blocks::default(),
        }
    }
}
