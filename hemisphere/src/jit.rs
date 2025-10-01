use crate::{System, system::Event};
use common::{
    Address, Primitive,
    arch::{Cpu, QuantizedType},
    util::boxed_array,
};
use interavl::IntervalTree;
use ppcjit::{
    Block,
    block::{GenericHook, GetRegistersHook, Hooks, ReadHook, ReadQuantizedHook, WriteHook},
};
use slotmap::{SlotMap, new_key_type};
use std::{collections::BTreeMap, ops::Range};
use tracing::{info, trace};

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

    extern "sysv64-unwind" fn read<P: Primitive>(
        ctx: &mut Context,
        addr: Address,
        value: &mut P,
    ) -> bool {
        if let Some(physical) = ctx.system.translate_data_addr(addr) {
            *value = ctx.system.read(physical);
            true
        } else {
            tracing::error!("failed to translate address {addr}");
            false
        }
    }

    extern "sysv64-unwind" fn write<P: Primitive>(
        ctx: &mut Context,
        addr: Address,
        value: P,
    ) -> bool {
        if let Some(physical) = ctx.system.translate_data_addr(addr) {
            ctx.system.write(physical, value);
            for i in 0..size_of::<P>() {
                ctx.mapping.invalidate(addr + i as u32);
            }

            true
        } else {
            tracing::error!("failed to translate address {addr}");
            false
        }
    }

    extern "sysv64-unwind" fn read_quantized(
        ctx: &mut Context,
        addr: Address,
        gqr: u8,
        value: &mut f64,
    ) -> bool {
        let _span = tracing::debug_span!("read quantized").entered();

        tracing::debug!("reading quantized at {addr}");
        if let Some(physical) = ctx.system.translate_data_addr(addr) {
            let gqr = ctx.system.cpu.supervisor.gq[gqr as usize].clone();

            let (mut read, scale) = match gqr.load_type() {
                QuantizedType::U8 => (ctx.system.read::<u8>(physical) as f64, true),
                QuantizedType::U16 => (ctx.system.read::<u16>(physical) as f64, true),
                QuantizedType::I8 => (ctx.system.read::<i8>(physical) as f64, true),
                QuantizedType::I16 => (ctx.system.read::<i16>(physical) as f64, true),
                _ => (
                    f32::from_bits(ctx.system.read::<u32>(physical)) as f64,
                    false,
                ),
            };

            tracing::debug!("read {read}");

            if scale {
                let scale = gqr.load_scale().value();
                read *= 2.0f64.powi(-scale as i32);
                tracing::debug!("scalign with {scale}, new value {read}");
            }

            *value = read;

            true
        } else {
            tracing::error!("failed to translate address {addr}");
            false
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
        ctx.system.update_decrementer();
    }

    extern "sysv64-unwind" fn dec_changed(ctx: &mut Context) {
        ctx.system.lazy.last_updated_dec = ctx.system.scheduler.elapsed_time_base();
        ctx.system
            .scheduler
            .retain(|e| e.event != Event::Decrementer);

        let dec = ctx.system.cpu.supervisor.misc.dec;
        trace!("decrementer changed to {dec}");

        ctx.system
            .scheduler
            .schedule(Event::Decrementer, dec as u64);
    }

    extern "sysv64-unwind" fn tb_read(ctx: &mut Context) {
        ctx.system.update_time_base();
    }

    extern "sysv64-unwind" fn tb_changed(ctx: &mut Context) {
        ctx.system.lazy.last_updated_tb = ctx.system.scheduler.elapsed_time_base();
        info!("time base changed to {}", ctx.system.cpu.supervisor.misc.tb);
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
            transmute::<_, ReadHook<i8>>(read::<i8> as extern "sysv64-unwind" fn(_, _, _) -> _);
        let write_i8 =
            transmute::<_, WriteHook<i8>>(write::<i8> as extern "sysv64-unwind" fn(_, _, _) -> _);
        let read_i16 =
            transmute::<_, ReadHook<i16>>(read::<i16> as extern "sysv64-unwind" fn(_, _, _) -> _);
        let write_i16 =
            transmute::<_, WriteHook<i16>>(write::<i16> as extern "sysv64-unwind" fn(_, _, _) -> _);
        let read_i32 =
            transmute::<_, ReadHook<i32>>(read::<i32> as extern "sysv64-unwind" fn(_, _, _) -> _);
        let write_i32 =
            transmute::<_, WriteHook<i32>>(write::<i32> as extern "sysv64-unwind" fn(_, _, _) -> _);
        let read_i64 =
            transmute::<_, ReadHook<i64>>(read::<i64> as extern "sysv64-unwind" fn(_, _, _) -> _);
        let write_i64 =
            transmute::<_, WriteHook<i64>>(write::<i64> as extern "sysv64-unwind" fn(_, _, _) -> _);
        let read_quantized = transmute::<_, ReadQuantizedHook>(
            read_quantized as extern "sysv64-unwind" fn(_, _, _, _) -> _,
        );

        let ibat_changed =
            transmute::<_, GenericHook>(ibat_changed as extern "sysv64-unwind" fn(_));
        let dbat_changed =
            transmute::<_, GenericHook>(dbat_changed as extern "sysv64-unwind" fn(_));

        let tb_read = transmute::<_, GenericHook>(tb_read as extern "sysv64-unwind" fn(_));
        let tb_changed = transmute::<_, GenericHook>(tb_changed as extern "sysv64-unwind" fn(_));

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
            read_i64,
            write_i64,
            read_quantized,

            ibat_changed,
            dbat_changed,

            tb_read,
            tb_changed,

            dec_read,
            dec_changed,
        }
    }
};

/// JIT configuration.
pub struct Config {
    /// Maximum number of instructions per JIT block.
    pub instr_per_block: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            instr_per_block: 512,
        }
    }
}

/// The JIT context.
pub struct JIT {
    pub config: Config,
    pub compiler: ppcjit::Compiler,
    pub blocks: Blocks,
}

impl JIT {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            compiler: ppcjit::Compiler::default(),
            blocks: Blocks::default(),
        }
    }
}
