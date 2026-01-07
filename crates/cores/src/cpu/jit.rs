mod table;

use hemisphere::{
    Address, Cycles, Primitive,
    cores::{CpuCore, Executed},
    gekko::{
        self, Cpu, QuantizedType,
        disasm::{Extensions, Ins},
    },
    system::{self, System},
};
use indexmap::IndexSet;
use ppcjit::{
    Block, FastmemLut,
    block::{BlockFn, Info, LinkData, Pattern},
    hooks::*,
};
use table::Table;

pub use ppcjit;
use util::boxed_array;

const TABLE_PRIMARY_BITS: usize = 12;
const TABLE_PRIMARY_COUNT: usize = 1 << TABLE_PRIMARY_BITS;
const TABLE_PRIMARY_MASK: usize = TABLE_PRIMARY_COUNT - 1;
const TABLE_SECONDARY_BITS: usize = 8;
const TABLE_SECONDARY_COUNT: usize = 1 << TABLE_SECONDARY_BITS;
const TABLE_SECONDARY_MASK: usize = TABLE_SECONDARY_COUNT - 1;
const TABLE_BLOCKS_BITS: usize = 10;
const TABLE_BLOCKS_COUNT: usize = 1 << TABLE_BLOCKS_BITS;
const TABLE_BLOCKS_MASK: usize = TABLE_BLOCKS_COUNT - 1;

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
    Table<Table<Table<Mapping, TABLE_BLOCKS_COUNT>, TABLE_SECONDARY_COUNT>, TABLE_PRIMARY_COUNT>;

#[inline(always)]
fn addr_to_table_idx(addr: Address) -> (usize, usize, usize) {
    let base = (addr.value() >> 2) as usize;
    (
        base >> (30 - TABLE_PRIMARY_BITS) & TABLE_PRIMARY_MASK,
        (base >> (30 - TABLE_PRIMARY_BITS - TABLE_SECONDARY_BITS)) & TABLE_SECONDARY_MASK,
        (base >> (30 - TABLE_PRIMARY_BITS - TABLE_SECONDARY_BITS - TABLE_BLOCKS_BITS))
            & TABLE_BLOCKS_MASK,
    )
}

const DEPS_TABLE_BITS: usize = 20;
const DEPS_TABLE_COUNT: usize = 1 << DEPS_TABLE_BITS;

#[inline(always)]
fn deps_page(addr: Address) -> usize {
    (addr.value() >> (32 - DEPS_TABLE_BITS)) as usize
}

/// A structure which keeps tracks of compiled [`Block`]s.
pub struct Blocks {
    storage: Vec<StoredBlock>,
    mappings: MappingTable,
    deps: Box<[IndexSet<Address>; DEPS_TABLE_COUNT]>,
    temp_deps: IndexSet<Address>,
}

impl Default for Blocks {
    fn default() -> Self {
        Self {
            storage: Default::default(),
            mappings: Default::default(),
            deps: boxed_array(IndexSet::new()),
            temp_deps: IndexSet::new(),
        }
    }
}

impl Blocks {
    fn insert_mapping(&mut self, addr: Address, mapping: Mapping) {
        let (idx0, idx1, idx2) = addr_to_table_idx(addr);
        let level1 = self.mappings.get_or_default(idx0);
        let level2 = level1.get_or_default(idx1);
        level2.insert(idx2, mapping);

        let start_page = deps_page(addr);
        let end_page = deps_page(addr + mapping.length);

        for page in start_page..=end_page {
            self.deps[page].insert(addr);
        }
    }

    fn remove_mapping_if_contains(&mut self, addr: Address, target: Address) -> Option<Mapping> {
        let (idx0, idx1, idx2) = addr_to_table_idx(addr);
        let level1 = self.mappings.get_mut(idx0)?;
        let level2 = level1.get_mut(idx1)?;
        let mapping = level2.get(idx2)?;

        let start = addr;
        let end = addr + mapping.length;

        if (start..=end).contains(&target) {
            let start_page = deps_page(addr);
            let end_page = deps_page(addr + mapping.length);

            for page in start_page..=end_page {
                self.deps[page].swap_remove(&addr);
            }

            level2.remove(idx2)
        } else {
            None
        }
    }

    /// Inserts a block into the storage and maps it to the given address.
    #[inline(always)]
    pub fn insert(&mut self, addr: Address, block: Block) -> BlockId {
        let length = 4 * block.meta().seq.len() as u32;
        let id = BlockId(self.storage.len());

        self.storage.push(StoredBlock {
            inner: block,
            links: Vec::new(),
        });

        self.insert_mapping(addr, Mapping { id, length });

        id
    }

    /// Returns the mapping at `addr`.
    #[inline(always)]
    pub fn get_mapping(&self, addr: Address) -> Option<Mapping> {
        let (idx0, idx1, idx2) = addr_to_table_idx(addr);
        let level1 = self.mappings.get(idx0)?;
        let level2 = level1.get(idx1)?;
        level2.get(idx2).copied()
    }

    /// Returns the block mapped to `addr`.
    #[inline(always)]
    pub fn get(&mut self, addr: Address) -> Option<&StoredBlock> {
        self.storage.get(self.get_mapping(addr)?.id.0)
    }

    /// Invalidate mappings that contain `addr`.
    pub fn invalidate(&mut self, target: Address) {
        let page = deps_page(target);
        if self.deps[page].is_empty() {
            return;
        }

        let mut temp_deps = std::mem::replace(&mut self.temp_deps, IndexSet::new());
        self.deps[page].clone_into(&mut temp_deps);

        for dep in temp_deps.iter() {
            let Some(mapping) = self.remove_mapping_if_contains(*dep, target) else {
                tracing::warn!(
                    "mapping {dep} is listed as dependent on page {page} but it does not exist"
                );
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
        self.mappings = Table::new();
        for deps in self.deps.iter_mut() {
            deps.clear();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitReason {
    None,
    IdleLooping,
}

const QUANTIZATION_FACTOR: [f64; 1 << 6] = {
    let mut result = [0.0; 1 << 6];

    let mut i = 0;
    loop {
        let scale = ((i as i8) << 2) >> 2;
        let exp = scale.unsigned_abs();
        let factor = if scale >= 0 {
            1.0 / ((1 << exp) as f64)
        } else {
            (1u64 << exp) as f64
        };

        result[i as usize] = factor;

        i += 1;
        if i >= (1 << 6) {
            break;
        }
    }

    result
};

/// Context to be passed in for execution of JIT blocks.
struct Context<'a> {
    /// The system state, so that the JIT block can operate on it.
    sys: &'a mut System,
    /// The block mapping, so that write operations can invalidate blocks.
    blocks: &'a mut Blocks,
    /// Amount of cycles we are trying to execute.
    target_cycles: u32,
    /// Maximum instructions we should execute.
    max_instructions: u32,
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
        if info.cycles >= ctx.target_cycles || info.instructions >= ctx.max_instructions {
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
        if let Some(mapping) = ctx.blocks.get_mapping(addr) {
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
        gqr: u8,
        value: &mut f64,
    ) -> u8 {
        let gqr = ctx.sys.cpu.supervisor.gq[gqr as usize];
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

        let scaled = read * QUANTIZATION_FACTOR[(scale as usize) & 0b0011_1111];
        *value = scaled;

        ty.size()
    }

    extern "sysv64-unwind" fn write_quantized(
        ctx: &mut Context,
        addr: Address,
        gqr: u8,
        value: f64,
    ) -> u8 {
        let gqr = ctx.sys.cpu.supervisor.gq[gqr as usize];
        let ty = gqr.store_type();

        let scale = if ty != QuantizedType::Float {
            gqr.store_scale().value()
        } else {
            0
        };

        let scaled = value * QUANTIZATION_FACTOR[(scale as usize) & 0b0011_1111];
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
        for offset in 0..32 {
            ctx.blocks.invalidate(aligned + offset);
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

        ctx.sys
            .cpu
            .supervisor
            .config
            .dma
            .lower
            .set_trigger(false)
            .set_flush(false);
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

pub struct Core {
    pub config: Config,
    pub compiler: ppcjit::Jit,
    pub blocks: Blocks,
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
            let physical = sys.translate_instr_addr(current)?;

            let ins = Ins::new(sys.read_phys_slow(physical), Extensions::gekko_broadway());
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
    ) -> Executed {
        let stored = self
            .blocks
            .get(sys.cpu.pc)
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
            target_cycles,
            max_instructions,

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
    ) -> Executed {
        let block = self
            .blocks
            .get(sys.cpu.pc)
            .filter(|b| b.inner.meta().seq.len() <= max_instructions as usize);

        if block.is_none() {
            // avoid trying to compile unimplemented instructions in debug mode
            let instructions = if cfg!(debug_assertions) {
                self.config.instr_per_block.min(max_instructions)
            } else {
                self.config.instr_per_block
            };

            let block = self.compile(sys, sys.cpu.pc, instructions);
            self.blocks.insert(sys.cpu.pc, block);
        }

        self.uncached_exec(sys, target_cycles, max_instructions)
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
            if let Some(stored) = self.blocks.get(sys.cpu.pc)
                && stored.inner.meta().pattern == Pattern::Call
                && let Some(dest) = stored.inner.meta().seq.is_call(sys.cpu.pc)
            {
                std::hint::cold_path();

                if let Some(func_block) = self.blocks.get(dest)
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
            let e = self.cached_exec(sys, target_cycles.0 as u32, max_instructions);
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
        self.uncached_exec(sys, u32::MAX, 1)
    }
}
