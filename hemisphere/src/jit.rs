use crate::{System, system::Event};
use gekko::{
    Address, Primitive,
    arch::{Cpu, QuantizedType},
    util::boxed_array,
};
use ppcjit::{
    Block,
    block::{
        GenericHook, GetRegistersHook, Hooks, ReadHook, ReadQuantizedHook, WriteHook,
        WriteQuantizedHook,
    },
};
use slotmap::{SlotMap, new_key_type};
use std::{collections::BTreeMap, ops::Range};

/// A page is 4096 (1^12) bytes. Therefore, there are 2^20 pages.
const PAGE_COUNT: usize = 1 << 20;
type PageLUT = Box<[u16; PAGE_COUNT]>;

new_key_type! {
    /// Identifier for a block in a [`Blocks`] storage.
    pub struct BlockId;
}

/// Mapping of addresses to JIT blocks.
pub struct BlockMapping {
    address: BTreeMap<Address, (BlockId, u32)>,
    page_lut: PageLUT,

    // caching stuff
    invalidation_buffer: Vec<Range<Address>>,
    last_query: Option<(Address, BlockId)>,
}

impl BlockMapping {
    fn insert(&mut self, range: Range<Address>, id: BlockId) {
        self.address
            .insert(range.start, (id, range.end.value() - range.start.value()));

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
            let (id, _) = *self.address.get(&addr)?;
            self.last_query = Some((addr, id));
            Some(id)
        }
    }

    /// Invalidates all block mappings that contain `addr`.
    #[inline(always)]
    pub fn invalidate(&mut self, addr: Address) {
        // check LUT first
        let page = addr.value() >> 12;

        #[expect(clippy::redundant_else, reason = "makes it clearer")]
        if self.page_lut[page as usize] == 0 {
            return;
        } else {
            std::hint::cold_path();
        }

        self.invalidation_buffer.extend(
            self.address
                .range(addr..)
                .map(|(addr, (_, len))| *addr..*addr + *len)
                .filter(|r| r.contains(&addr)),
        );

        for range in self.invalidation_buffer.drain(..) {
            self.address.remove(&range.start);

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

    /// Invalidates all mappings.
    pub fn clear(&mut self) {
        self.address.clear();
        self.page_lut.fill(0);

        self.invalidation_buffer.clear();
        self.last_query = None;
    }
}

impl Default for BlockMapping {
    fn default() -> Self {
        Self {
            address: BTreeMap::default(),
            page_lut: boxed_array(0),

            invalidation_buffer: Vec::with_capacity(64),
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
    /// The system state, so that the JIT block can operate on it.
    pub system: &'a mut System,
    /// The block mapping, so that write operations can invalidate blocks.
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
            // tracing::debug!(
            //     "reading from logical {addr}, physical {physical}: 0x{:X?}",
            //     value
            // );
            true
        } else {
            tracing::error!(pc = ?ctx.system.cpu.pc, "failed to translate address {addr}");
            false
        }
    }

    extern "sysv64-unwind" fn write<P: Primitive>(
        ctx: &mut Context,
        addr: Address,
        value: P,
    ) -> bool {
        let Some(physical) = ctx.system.translate_data_addr(addr) else {
            tracing::error!(pc = ?ctx.system.cpu.pc, "failed to translate address {addr}");
            return false;
        };

        // tracing::debug!(
        //     "writing to logical {addr}, physical {physical}: 0x{:X?}",
        //     value
        // );

        ctx.system.write(physical, value);
        for i in 0..size_of::<P>() {
            ctx.mapping.invalidate(addr + i as u32);
        }

        true
    }

    extern "sysv64-unwind" fn read_quantized(
        ctx: &mut Context,
        addr: Address,
        gqr: u8,
        value: &mut f64,
    ) -> u8 {
        let Some(physical) = ctx.system.translate_data_addr(addr) else {
            tracing::error!("failed to translate address {addr}");
            return 0;
        };

        let gqr = ctx.system.cpu.supervisor.gq[gqr as usize].clone();
        let scale = if gqr.load_type() != QuantizedType::Float {
            gqr.load_scale().value()
        } else {
            0
        };

        let read = match gqr.load_type() {
            QuantizedType::U8 => ctx.system.read::<u8>(physical) as f64,
            QuantizedType::U16 => ctx.system.read::<u16>(physical) as f64,
            QuantizedType::I8 => ctx.system.read::<i8>(physical) as f64,
            QuantizedType::I16 => ctx.system.read::<i16>(physical) as f64,
            _ => f32::from_bits(ctx.system.read::<u32>(physical)) as f64,
        };

        let scaled = read * 2.0f64.powi(-scale as i32);
        *value = scaled;

        gqr.load_type().size()
    }

    extern "sysv64-unwind" fn write_quantized(
        ctx: &mut Context,
        addr: Address,
        gqr: u8,
        value: f64,
    ) -> u8 {
        let Some(physical) = ctx.system.translate_data_addr(addr) else {
            tracing::error!("failed to translate address {addr}");
            return 0;
        };

        let gqr = ctx.system.cpu.supervisor.gq[gqr as usize].clone();
        let scale = if gqr.store_type() != QuantizedType::Float {
            gqr.store_scale().value()
        } else {
            0
        };
        let scaled = value * 2.0f64.powi(-scale as i32);

        match gqr.store_type() {
            QuantizedType::U8 => ctx.system.write(physical, scaled as u8),
            QuantizedType::U16 => ctx.system.write(physical, scaled as u16),
            QuantizedType::I8 => ctx.system.write(physical, scaled as i8),
            QuantizedType::I16 => ctx.system.write(physical, scaled as i16),
            _ => ctx.system.write(physical, (scaled as f32).to_bits()),
        }

        gqr.store_type().size()
    }

    extern "sysv64-unwind" fn msr_changed(ctx: &mut Context) {
        ctx.system.scheduler.schedule_now(Event::CheckInterrupts);
    }

    extern "sysv64-unwind" fn ibat_changed(ctx: &mut Context) {
        tracing::info!("ibats changed - clearing blocks mapping and rebuilding ibat lut");
        ctx.mapping.clear();
        ctx.system
            .mmu
            .build_instr_bat_lut(&ctx.system.cpu.supervisor.memory.ibat);
    }

    extern "sysv64-unwind" fn dbat_changed(ctx: &mut Context) {
        tracing::info!("dbats changed - rebuilding dbat lut");
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
        tracing::trace!("decrementer changed to {dec}");

        ctx.system
            .scheduler
            .schedule(Event::Decrementer, dec as u64);
    }

    extern "sysv64-unwind" fn tb_read(ctx: &mut Context) {
        ctx.system.update_time_base();
    }

    extern "sysv64-unwind" fn tb_changed(ctx: &mut Context) {
        ctx.system.lazy.last_updated_tb = ctx.system.scheduler.elapsed_time_base();
        tracing::info!("time base changed to {}", ctx.system.cpu.supervisor.misc.tb);
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
        let write_quantized = transmute::<_, WriteQuantizedHook>(
            write_quantized as extern "sysv64-unwind" fn(_, _, _, _) -> _,
        );

        let msr_changed = transmute::<_, GenericHook>(msr_changed as extern "sysv64-unwind" fn(_));

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
            write_quantized,

            msr_changed,

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
