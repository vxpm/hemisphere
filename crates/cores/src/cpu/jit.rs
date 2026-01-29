mod table;

use indexmap::IndexSet;
use lazuli::cores::{CpuCore, Executed};
use lazuli::gekko::disasm::{Extensions, Ins};
use lazuli::gekko::{self, Cpu, DEQUANTIZATION_LUT, QUANTIZATION_LUT, QuantReg, QuantizedType};
use lazuli::system::{self, System};
use lazuli::{Address, Cycles, Primitive};
use ppcjit::block::{BlockFn, Info, LinkData, Pattern};
use ppcjit::hooks::*;
use ppcjit::{Block, FastmemLut};
use table::Table;

#[rustfmt::skip]
pub use ppcjit;

const MAP_TBL_L0_BITS: usize = 12;
const MAP_TBL_L0_COUNT: usize = 1 << MAP_TBL_L0_BITS;
const MAP_TBL_L0_MASK: usize = MAP_TBL_L0_COUNT - 1;
const MAP_TBL_L1_BITS: usize = 8;
const MAP_TBL_L1_COUNT: usize = 1 << MAP_TBL_L1_BITS;
const MAP_TBL_L1_MASK: usize = MAP_TBL_L1_COUNT - 1;
const MAP_TBL_L2_BITS: usize = 10;
const MAP_TBL_L2_COUNT: usize = 1 << MAP_TBL_L2_BITS;
const MAP_TBL_L2_MASK: usize = MAP_TBL_L2_COUNT - 1;

/// Identifier for a block in a [`Blocks`] storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockId(usize);

pub struct StoredBlock {
    pub inner: Block,
    pub links: Vec<*mut Option<LinkData>>,
}

// TODO: this is problematic
unsafe impl Send for StoredBlock {}

#[derive(Debug, Clone, Copy)]
pub struct Mapping {
    pub id: BlockId,
    pub length: u32,
}

type MappingTable =
    Table<Table<Table<Mapping, MAP_TBL_L2_COUNT>, MAP_TBL_L1_COUNT>, MAP_TBL_L0_COUNT>;

#[inline(always)]
fn addr_to_mapping_idx(addr: Address) -> (usize, usize, usize) {
    let base = (addr.value() >> 2) as usize;
    (
        base >> (30 - MAP_TBL_L0_BITS) & MAP_TBL_L0_MASK,
        (base >> (30 - MAP_TBL_L0_BITS - MAP_TBL_L1_BITS)) & MAP_TBL_L1_MASK,
        (base >> (30 - MAP_TBL_L0_BITS - MAP_TBL_L1_BITS - MAP_TBL_L2_BITS)) & MAP_TBL_L2_MASK,
    )
}

const DEPS_TBL_L0_BITS: usize = 12;
const DEPS_TBL_L0_COUNT: usize = 1 << DEPS_TBL_L0_BITS;
const DEPS_TBL_L0_MASK: usize = DEPS_TBL_L0_COUNT - 1;
const DEPS_TBL_L1_BITS: usize = 8;
const DEPS_TBL_L1_COUNT: usize = 1 << DEPS_TBL_L1_BITS;
const DEPS_TBL_L1_MASK: usize = DEPS_TBL_L1_COUNT - 1;

#[inline(always)]
fn deps_page_base(addr: Address) -> Address {
    Address(addr.value() >> 12)
}

#[inline(always)]
fn addr_to_deps_idx(addr: Address) -> (usize, usize) {
    let base = deps_page_base(addr).value() as usize;
    (
        base >> (20 - DEPS_TBL_L0_BITS) & DEPS_TBL_L0_MASK,
        (base >> (20 - DEPS_TBL_L0_BITS - DEPS_TBL_L1_BITS)) & DEPS_TBL_L1_MASK,
    )
}

type DepsTable = Table<Table<IndexSet<Address>, DEPS_TBL_L1_COUNT>, DEPS_TBL_L0_COUNT>;

/// A structure which keeps tracks of compiled [`Block`]s.
pub struct Blocks {
    storage: Vec<StoredBlock>,
    logical_mappings: MappingTable,
    physical_mappings: MappingTable,
    logical_deps: DepsTable,
    physical_deps: DepsTable,
    temp_deps: IndexSet<Address>,
}

impl Default for Blocks {
    fn default() -> Self {
        Self {
            storage: Default::default(),
            logical_mappings: Default::default(),
            physical_mappings: Default::default(),
            logical_deps: Default::default(),
            physical_deps: Default::default(),
            temp_deps: IndexSet::new(),
        }
    }
}

struct MappingNotFoundError;

impl Blocks {
    fn insert_mapping(&mut self, logical: bool, addr: Address, mapping: Mapping) {
        let (mappings, deps) = if logical {
            (&mut self.logical_mappings, &mut self.logical_deps)
        } else {
            (&mut self.physical_mappings, &mut self.physical_deps)
        };

        let (idx0, idx1, idx2) = addr_to_mapping_idx(addr);
        let level1 = mappings.get_or_default(idx0);
        let level2 = level1.get_or_default(idx1);
        level2.insert(idx2, mapping);

        let count = mapping.length.div_ceil(4096);
        let mut current = addr;
        for _ in 0..count {
            let (idx0, idx1) = addr_to_deps_idx(current);
            let level1 = deps.get_or_default(idx0);
            let deps = level1.get_or_default(idx1);
            deps.insert(addr);
            current += 4096;
        }
    }

    fn remove_mapping_if_contains(
        &mut self,
        logical: bool,
        addr: Address,
        target: Address,
    ) -> Result<Option<Mapping>, MappingNotFoundError> {
        let (mappings, deps) = if logical {
            (&mut self.logical_mappings, &mut self.logical_deps)
        } else {
            (&mut self.physical_mappings, &mut self.physical_deps)
        };

        let (idx0, idx1, idx2) = addr_to_mapping_idx(addr);
        let level1 = mappings.get_mut(idx0).ok_or(MappingNotFoundError)?;
        let level2 = level1.get_mut(idx1).ok_or(MappingNotFoundError)?;
        let mapping = level2.get(idx2).ok_or(MappingNotFoundError)?;

        let start = addr;
        let end = addr + mapping.length;

        if (start..=end).contains(&target) {
            let count = mapping.length.div_ceil(4096);
            let mut current = addr;
            for _ in 0..count {
                let (idx0, idx1) = addr_to_deps_idx(current);
                let level1 = deps.get_or_default(idx0);
                let deps = level1.get_or_default(idx1);
                deps.swap_remove(&addr);
                current += 4096;
            }

            Ok(Some(level2.remove(idx2).unwrap()))
        } else {
            Ok(None)
        }
    }

    /// Inserts a block into the storage and maps it to the given address.
    #[inline(always)]
    pub fn insert(&mut self, logical: bool, addr: Address, block: Block) -> BlockId {
        let length = 4 * block.meta().seq.len() as u32;
        let id = BlockId(self.storage.len());

        self.storage.push(StoredBlock {
            inner: block,
            links: Vec::new(),
        });

        self.insert_mapping(logical, addr, Mapping { id, length });

        id
    }

    /// Returns the mapping at `addr`.
    #[inline(always)]
    pub fn get_mapping(&self, logical: bool, addr: Address) -> Option<Mapping> {
        let mappings = if logical {
            &self.logical_mappings
        } else {
            &self.physical_mappings
        };

        let (idx0, idx1, idx2) = addr_to_mapping_idx(addr);
        let level1 = mappings.get(idx0)?;
        let level2 = level1.get(idx1)?;
        level2.get(idx2).copied()
    }

    /// Returns the block mapped to `addr`.
    #[inline(always)]
    pub fn get(&mut self, logical: bool, addr: Address) -> Option<&StoredBlock> {
        self.storage.get(self.get_mapping(logical, addr)?.id.0)
    }

    /// Invalidate mappings that contain `addr`.
    pub fn invalidate(&mut self, logical: bool, target: Address) {
        let deps = if logical {
            &mut self.logical_deps
        } else {
            &mut self.physical_deps
        };

        let (idx0, idx1) = addr_to_deps_idx(target);
        let Some(level1) = deps.get(idx0) else {
            return;
        };
        let Some(deps) = level1.get(idx1) else {
            return;
        };

        if deps.is_empty() {
            return;
        }

        let mut temp_deps = std::mem::replace(&mut self.temp_deps, IndexSet::new());
        deps.clone_into(&mut temp_deps);

        for dep in temp_deps.iter() {
            let mapping = match self.remove_mapping_if_contains(logical, *dep, target) {
                Ok(mapping) => mapping,
                Err(_) => {
                    let page = deps_page_base(target);
                    panic!(
                        "mapping {dep} is listed as dependent on page {page} but it does not exist"
                    );
                }
            };

            let Some(mapping) = mapping else {
                continue;
            };

            let block = &mut self.storage[mapping.id.0];
            for link in block.links.drain(..) {
                let link = unsafe { link.as_mut().unwrap() };
                *link = None;
            }
        }

        temp_deps.clear();
        self.temp_deps = temp_deps;
    }

    /// Clears all mappings.
    pub fn clear(&mut self) {
        self.logical_mappings = Table::new();
        self.physical_mappings = Table::new();
        self.logical_deps = Table::new();
        self.physical_deps = Table::new();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitReason {
    None,
    IdleLooping,
}

/// Context to be passed in for execution of JIT blocks.
struct Context<'a> {
    /// The system state, so that the JIT block can operate on it.
    sys: &'a mut System,
    /// The block mapping, so that write operations can invalidate blocks.
    blocks: &'a mut Blocks,
    /// ICache
    icache: &'a mut ICache,
    /// Amount of cycles we are trying to execute.
    target_cycles: u32,
    /// Maximum instructions we should execute.
    max_instructions: u32,
    /// Whether to forcely disable following links.
    force_no_link: bool,
    /// Last followed link.
    last_followed_link: Option<BlockFn>,
    /// Reason for exit.
    exit_reason: ExitReason,
}

const CTX_HOOKS: Hooks = {
    extern "sysv64-unwind" fn get_registers<'a>(ctx: &'a mut Context) -> &'a mut Cpu {
        &mut ctx.sys.cpu
    }

    extern "sysv64-unwind" fn get_fastmem<'a>(ctx: &'a mut Context) -> &'a FastmemLut {
        if ctx.sys.cpu.supervisor.config.msr.data_addr_translation() {
            ctx.sys.mem.data_fastmem_lut_logical()
        } else {
            ctx.sys.mem.data_fastmem_lut_physical()
        }
    }

    extern "sysv64-unwind" fn follow_link(
        info: &Info,
        ctx: &mut Context,
        link_data: &mut Option<LinkData>,
    ) -> bool {
        // if we have reached cycle or instruction limit, don't follow links, just exit.
        if ctx.force_no_link
            || info.cycles >= ctx.target_cycles
            || info.instructions >= ctx.max_instructions
        {
            ctx.last_followed_link = None;
            return false;
        }

        let Some(link_data) = link_data else {
            return true;
        };

        // otherwise, detect whether we are idle looping and exit too
        let follow = match link_data.pattern {
            Pattern::IdleBasic | Pattern::IdleVolatileRead => {
                if ctx.last_followed_link == Some(link_data.block) {
                    ctx.exit_reason = ExitReason::IdleLooping;
                    false
                } else {
                    true
                }
            }
            _ => true,
        };

        // if not idle looping, then sure, follow link
        ctx.last_followed_link = Some(link_data.block);
        follow
    }

    extern "sysv64-unwind" fn try_link(
        ctx: &mut Context,
        addr: Address,
        link_data: &mut Option<LinkData>,
    ) {
        debug_assert!(link_data.is_none());
        let logical = ctx.sys.cpu.supervisor.config.msr.instr_addr_translation();
        if let Some(mapping) = ctx.blocks.get_mapping(logical, addr) {
            let stored = ctx.blocks.storage.get_mut(mapping.id.0).unwrap();
            *link_data = Some(LinkData {
                block: stored.inner.as_ptr(),
                pattern: stored.inner.meta().pattern,
            });

            stored.links.push(&raw mut *link_data);
        }
    }

    extern "sysv64-unwind" fn read<P: Primitive>(
        ctx: &mut Context,
        addr: Address,
        value: &mut P,
    ) -> bool {
        if let Some(read) = ctx.sys.read_slow(addr) {
            *value = read;
            true
        } else {
            std::hint::cold_path();
            tracing::error!(pc = ?ctx.sys.cpu.pc, "failed to translate address {addr}");
            false
        }
    }

    extern "sysv64-unwind" fn write<P: Primitive>(
        ctx: &mut Context,
        addr: Address,
        value: P,
    ) -> bool {
        if ctx.sys.write_slow(addr, value) {
            true
        } else {
            std::hint::cold_path();
            tracing::error!(pc = ?ctx.sys.cpu.pc, "failed to translate address {addr}");
            false
        }
    }

    extern "sysv64-unwind" fn read_quantized(
        ctx: &mut Context,
        addr: Address,
        gqr: QuantReg,
        value: &mut f64,
    ) -> u8 {
        let ty = gqr.load_type();
        let scale = if ty != QuantizedType::Float {
            gqr.load_scale().value()
        } else {
            0
        };

        let read = match ty {
            QuantizedType::U8 => ctx.sys.read::<u8>(addr).map(|x| x as f64),
            QuantizedType::U16 => ctx.sys.read::<u16>(addr).map(|x| x as f64),
            QuantizedType::I8 => ctx.sys.read::<i8>(addr).map(|x| x as f64),
            QuantizedType::I16 => ctx.sys.read::<i16>(addr).map(|x| x as f64),
            _ => ctx.sys.read::<u32>(addr).map(|x| f32::from_bits(x) as f64),
        };

        let Some(read) = read else {
            std::hint::cold_path();
            tracing::error!("failed to translate address {addr}");
            return 0;
        };

        let scaled = read * DEQUANTIZATION_LUT[(scale as usize) & 0b0011_1111];
        *value = scaled;

        ty.size()
    }

    extern "sysv64-unwind" fn write_quantized(
        ctx: &mut Context,
        addr: Address,
        gqr: QuantReg,
        value: f64,
    ) -> u8 {
        let ty = gqr.store_type();
        let scale = if ty != QuantizedType::Float {
            gqr.store_scale().value()
        } else {
            0
        };

        let scaled = value * QUANTIZATION_LUT[(scale as usize) & 0b0011_1111];
        let success = match ty {
            QuantizedType::U8 => ctx.sys.write(addr, scaled as u8),
            QuantizedType::U16 => ctx.sys.write(addr, scaled as u16),
            QuantizedType::I8 => ctx.sys.write(addr, scaled as i8),
            QuantizedType::I16 => ctx.sys.write(addr, scaled as i16),
            _ => ctx.sys.write(addr, (scaled as f32).to_bits()),
        };

        if !success {
            std::hint::cold_path();
            tracing::error!("failed to translate address {addr}");
            return 0;
        }

        ty.size()
    }

    extern "sysv64-unwind" fn invalidate_icache(ctx: &mut Context, addr: Address) {
        let aligned = Address(addr.value() & !0x1F);
        let is_logical = ctx.sys.cpu.supervisor.config.msr.instr_addr_translation();

        if is_logical {
            for offset in 0..32 {
                let logical = aligned + offset;
                let physical = ctx.sys.translate_inst_addr(logical);

                ctx.blocks.invalidate(true, logical);
                if let Some(physical) = physical {
                    ctx.blocks.invalidate(false, physical);
                }
            }

            if let Some(physical) = ctx.sys.translate_inst_addr(aligned) {
                let (idx0, idx1, idx2) = addr_to_icache_idx(physical);
                if let Some(level1) = ctx.icache.get_mut(idx0)
                    && let Some(level2) = level1.get_mut(idx1)
                {
                    level2.remove(idx2);
                }
            }
        } else {
            for offset in 0..32 {
                let physical = aligned + offset;
                ctx.blocks.invalidate(false, physical);
            }

            let (idx0, idx1, idx2) = addr_to_icache_idx(addr);
            if let Some(level1) = ctx.icache.get_mut(idx0)
                && let Some(level2) = level1.get_mut(idx1)
            {
                level2.remove(idx2);
            }
        }
    }

    extern "sysv64-unwind" fn dcache_dma(ctx: &mut Context) {
        let dma = ctx.sys.cpu.supervisor.config.dma.clone();

        if dma.lower.trigger() {
            let regions = ctx.sys.mem.regions();
            let ram =
                &mut regions.ram[dma.mem_address().value() as usize..][..dma.length() as usize];
            let l2c = &mut regions.l2c[dma.cache_address().value() as usize - 0xE000_0000..]
                [..dma.length() as usize];

            debug_assert!(dma.length() <= 4096);

            match dma.lower.direction() {
                gekko::DmaDirection::FromCacheToRam => {
                    ram.copy_from_slice(l2c);
                }
                gekko::DmaDirection::FromRamToCache => {
                    l2c.copy_from_slice(ram);
                }
            }
        }

        ctx.sys.cpu.supervisor.config.dma.lower.set_trigger(false);
        ctx.sys.cpu.supervisor.config.dma.lower.set_flush(false);
    }

    extern "sysv64-unwind" fn clear_icache(ctx: &mut Context) {
        *ctx.icache = Table::new();
    }

    extern "sysv64-unwind" fn msr_changed(ctx: &mut Context) {
        ctx.sys.scheduler.schedule_now(system::pi::check_interrupts);
    }

    extern "sysv64-unwind" fn ibat_changed(ctx: &mut Context) {
        tracing::info!("ibats changed - clearing blocks mapping and rebuilding ibat lut");
        ctx.blocks.clear();
        ctx.sys
            .mem
            .build_instr_bat_lut(&ctx.sys.cpu.supervisor.memory.ibat);
    }

    extern "sysv64-unwind" fn dbat_changed(ctx: &mut Context) {
        tracing::info!("dbats changed - rebuilding dbat lut");
        ctx.sys
            .mem
            .build_data_bat_lut(&ctx.sys.cpu.supervisor.memory.dbat);
    }

    extern "sysv64-unwind" fn dec_read(ctx: &mut Context) {
        ctx.sys.update_decrementer();
    }

    extern "sysv64-unwind" fn dec_changed(ctx: &mut Context) {
        ctx.sys.lazy.last_updated_dec = ctx.sys.scheduler.elapsed_time_base();
        ctx.sys.scheduler.cancel(System::decrementer_overflow);

        let dec = ctx.sys.cpu.supervisor.misc.dec;
        tracing::trace!("decrementer changed to {dec}");

        ctx.sys
            .scheduler
            .schedule(dec as u64, System::decrementer_overflow);
    }

    extern "sysv64-unwind" fn tb_read(ctx: &mut Context) {
        ctx.sys.update_time_base();
    }

    extern "sysv64-unwind" fn tb_changed(ctx: &mut Context) {
        ctx.sys.lazy.last_updated_tb = ctx.sys.scheduler.elapsed_time_base();
        tracing::info!("time base changed to {}", ctx.sys.cpu.supervisor.misc.tb);
    }

    #[expect(
        clippy::missing_transmute_annotations,
        reason = "unnecessary - the definitions are above"
    )]
    unsafe {
        use std::mem::transmute;

        let get_registers =
            transmute::<_, GetRegistersHook>(get_registers as extern "sysv64-unwind" fn(_) -> _);
        let get_fastmem =
            transmute::<_, GetFastmemHook>(get_fastmem as extern "sysv64-unwind" fn(_) -> _);

        let follow_link =
            transmute::<_, FollowLinkHook>(follow_link as extern "sysv64-unwind" fn(_, _, _) -> _);
        let try_link = transmute::<_, TryLinkHook>(try_link as extern "sysv64-unwind" fn(_, _, _));

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

        let invalidate_icache =
            transmute::<_, InvalidateICache>(invalidate_icache as extern "sysv64-unwind" fn(_, _));
        let clear_icache =
            transmute::<_, GenericHook>(clear_icache as extern "sysv64-unwind" fn(_));
        let dcache_dma = transmute::<_, GenericHook>(dcache_dma as extern "sysv64-unwind" fn(_));

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
            get_fastmem,

            follow_link,
            try_link,

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

            invalidate_icache,
            clear_icache,
            dcache_dma,

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
    /// Code generation settings.
    pub jit_settings: ppcjit::Settings,
}

const ICACHE_L0_BITS: usize = 11;
const ICACHE_L0_COUNT: usize = 1 << ICACHE_L0_BITS;
const ICACHE_L0_MASK: usize = ICACHE_L0_COUNT - 1;
const ICACHE_L1_BITS: usize = 8;
const ICACHE_L1_COUNT: usize = 1 << ICACHE_L1_BITS;
const ICACHE_L1_MASK: usize = ICACHE_L1_COUNT - 1;
const ICACHE_L2_BITS: usize = 8;
const ICACHE_L2_COUNT: usize = 1 << ICACHE_L2_BITS;
const ICACHE_L2_MASK: usize = ICACHE_L2_COUNT - 1;

type CacheLine = [u32; 8];
type ICache = Table<Table<Table<CacheLine, ICACHE_L2_COUNT>, ICACHE_L1_COUNT>, ICACHE_L0_COUNT>;

#[inline(always)]
fn addr_to_icache_idx(addr: Address) -> (usize, usize, usize) {
    let base = (addr.value() >> 5) as usize;
    (
        base >> (27 - ICACHE_L0_BITS) & ICACHE_L0_MASK,
        (base >> (27 - ICACHE_L0_BITS - ICACHE_L1_BITS)) & ICACHE_L1_MASK,
        (base >> (27 - ICACHE_L0_BITS - ICACHE_L1_BITS - ICACHE_L2_BITS)) & ICACHE_L2_MASK,
    )
}

pub struct Core {
    pub config: Config,
    pub compiler: ppcjit::Jit,
    pub blocks: Blocks,
    pub icache: ICache,
}

fn closest_breakpoint(pc: Address, breakpoints: &[Address]) -> Address {
    let mut closest_breakpoint = Address(pc.value().saturating_add(u32::MAX));
    let mut closest_distance = closest_breakpoint.value() - pc.value();
    for breakpoint in breakpoints.iter().copied() {
        let distance = breakpoint.value().checked_sub(pc.value());
        if let Some(distance) = distance
            && distance <= closest_distance
            && distance != 0
        {
            closest_breakpoint = breakpoint;
            closest_distance = distance;
        }
    }

    closest_breakpoint
}

impl Core {
    pub fn new(config: Config) -> Self {
        let compiler = ppcjit::Jit::new(config.jit_settings.clone(), CTX_HOOKS);

        Self {
            config,
            compiler,
            blocks: Blocks::default(),
            icache: Default::default(),
        }
    }

    /// Compiles a sequence of at most `limit` instructions starting at `addr` into a JIT block.
    fn compile(&mut self, sys: &mut System, addr: Address, limit: u32) -> ppcjit::Block {
        let _span = tracing::trace_span!("compiling new block", addr = ?sys.cpu.pc).entered();

        let mut count = 0;
        let instructions = std::iter::from_fn(|| {
            if count >= limit {
                return None;
            }

            let current = addr + 4 * count;
            let Some(physical) = sys.translate_inst_addr(current) else {
                println!("failed to translate {current} at {}", addr);
                return None;
            };

            let (idx0, idx1, idx2) = addr_to_icache_idx(physical);
            let level1 = self.icache.get_or_default(idx0);
            let level2 = level1.get_or_default(idx1);
            let cacheline = match level2.get(idx2) {
                Some(cacheline) => cacheline,
                None => {
                    let base = physical.align_down(32);

                    let mut cacheline = [0; 8];
                    for index in 0..8 {
                        cacheline[index] = sys.read_phys_slow::<u32>(base + 4 * index as u32);
                    }

                    level2.insert(idx2, cacheline)
                }
            };

            let offset = (physical.value() % 32) / 4;
            let ins = Ins::new(cacheline[offset as usize], Extensions::gekko_broadway());
            count += 1;

            Some(ins)
        });

        let block = match self.compiler.build(instructions) {
            Ok(b) => b,
            Err(e) => match e {
                ppcjit::BuildError::EmptyBlock => panic!("built empty block at pc {}", sys.cpu.pc),
                ppcjit::BuildError::Builder { source } => panic!("block builder error: {}", source),
                ppcjit::BuildError::Codegen { source } => panic!("block codegen error: {}", source),
            },
        };

        tracing::trace!(
            instructions = block.meta().seq.len(),
            "block sequence built"
        );

        block
    }

    #[inline(always)]
    fn uncached_exec(
        &mut self,
        sys: &mut System,
        target_cycles: u32,
        max_instructions: u32,
        force_no_link: bool,
    ) -> Executed {
        let logical = sys.cpu.supervisor.config.msr.instr_addr_translation();
        let stored = self
            .blocks
            .get(logical, sys.cpu.pc)
            .filter(|b| b.inner.meta().seq.len() <= max_instructions as usize);

        let compiled: ppcjit::Block;
        let block = match stored {
            Some(stored) => stored.inner.as_ptr(),
            None => {
                std::hint::cold_path();

                compiled = self.compile(sys, sys.cpu.pc, max_instructions);
                compiled.as_ptr()
            }
        };

        let mut ctx = Context {
            sys,
            blocks: &mut self.blocks,
            icache: &mut self.icache,
            target_cycles,
            max_instructions,
            force_no_link,

            last_followed_link: None,
            exit_reason: ExitReason::None,
        };

        let info = unsafe {
            self.compiler
                .call(&raw mut ctx as *mut ppcjit::hooks::Context, block)
        };

        let cycles = if ctx.exit_reason == ExitReason::IdleLooping {
            std::hint::cold_path();
            Cycles(target_cycles as u64)
        } else {
            Cycles(info.cycles as u64)
        };

        Executed {
            instructions: info.instructions,
            cycles,
            hit_breakpoint: false,
        }
    }

    fn cached_exec(
        &mut self,
        sys: &mut System,
        target_cycles: u32,
        max_instructions: u32,
        force_no_link: bool,
    ) -> Executed {
        let logical = sys.cpu.supervisor.config.msr.instr_addr_translation();
        let block = self
            .blocks
            .get(logical, sys.cpu.pc)
            .filter(|b| b.inner.meta().seq.len() <= max_instructions as usize);

        if block.is_none() {
            // avoid trying to compile unimplemented instructions in debug mode
            let instructions = if cfg!(debug_assertions) {
                self.config.instr_per_block.min(max_instructions)
            } else {
                self.config.instr_per_block
            };

            let block = self.compile(sys, sys.cpu.pc, instructions);
            self.blocks.insert(logical, sys.cpu.pc, block);
        }

        self.uncached_exec(sys, target_cycles, max_instructions, force_no_link)
    }

    fn exec_inner<const BREAKPOINTS: bool>(
        &mut self,
        sys: &mut System,
        cycles: Cycles,
        breakpoints: &[Address],
    ) -> Executed {
        let mut executed = Executed::default();
        while executed.cycles < cycles {
            // detect mailbox idle loop
            let logical = sys.cpu.supervisor.config.msr.instr_addr_translation();
            if let Some(stored) = self.blocks.get(logical, sys.cpu.pc)
                && stored.inner.meta().pattern == Pattern::Call
                && let Some(dest) = stored.inner.meta().seq.is_call(sys.cpu.pc)
            {
                std::hint::cold_path();

                if let Some(func_block) = self.blocks.get(logical, dest)
                    && func_block.inner.meta().pattern == Pattern::GetMailboxStatusFunc
                    && sys.dsp.cpu_mailbox.status()
                {
                    std::hint::cold_path();
                    executed.cycles = cycles;
                    executed.instructions = 1;
                    break;
                }
            }

            let max_instructions = if BREAKPOINTS {
                let closest_breakpoint = closest_breakpoint(sys.cpu.pc, breakpoints);
                (closest_breakpoint.value() - sys.cpu.pc.value()) / 4
            } else {
                u32::MAX
            };

            // execute
            let target_cycles = cycles - executed.cycles;
            let e = self.cached_exec(sys, target_cycles.0 as u32, max_instructions, BREAKPOINTS);
            executed.instructions += e.instructions;
            executed.cycles += e.cycles;

            if BREAKPOINTS && breakpoints.contains(&sys.cpu.pc) {
                executed.hit_breakpoint = true;
                break;
            }
        }

        executed
    }
}

impl CpuCore for Core {
    fn exec(&mut self, sys: &mut System, cycles: Cycles, breakpoints: &[Address]) -> Executed {
        if breakpoints.is_empty() {
            self.exec_inner::<false>(sys, cycles, &[])
        } else {
            self.exec_inner::<true>(sys, cycles, breakpoints)
        }
    }

    fn step(&mut self, sys: &mut System) -> Executed {
        self.uncached_exec(sys, u32::MAX, 1, true)
    }
}
