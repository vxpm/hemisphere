use hemisphere::{
    Address, Cycles, Primitive,
    cores::{CpuCore, Executed},
    gekko::{
        self, Cpu, QuantizedType,
        disasm::{Extensions, Ins},
    },
    system::{self, System},
};
use indexmap::IndexMap;
use ppcjit::{
    Block,
    block::{BlockFn, Info, LinkData, Pattern, Trampoline},
    hooks::*,
};
use rustc_hash::FxBuildHasher;
use seq_macro::seq;
use slotmap::{SlotMap, new_key_type};
use std::{cell::Cell, ops::Range};

pub use ppcjit;

type FxIndexMap<K, V> = IndexMap<K, V, FxBuildHasher>;

const PAGE_SHIFT: usize = 12;
const PAGE_COUNT: usize = 1 << (32 - PAGE_SHIFT);

new_key_type! {
    /// Identifier for a block in a [`Blocks`] storage.
    pub struct BlockId;
}

#[derive(Debug, Clone)]
struct Mapping {
    id: BlockId,
    length: u32,
}

/// Mapping of addresses to JIT blocks.
pub struct BlockMapping {
    tree_map: FxIndexMap<Address, Mapping>,
    overlap_lut: Box<[u16; PAGE_COUNT]>,

    // caching stuff
    to_remove: Vec<Address>,
    last_query: Cell<Option<(Address, BlockId)>>,
}

impl Default for BlockMapping {
    fn default() -> Self {
        Self {
            tree_map: IndexMap::default(),
            overlap_lut: util::boxed_array(0),

            to_remove: Vec::with_capacity(128),
            last_query: Cell::new(None),
        }
    }
}

impl BlockMapping {
    fn insert(&mut self, range: Range<Address>, id: BlockId) {
        self.tree_map.insert(
            range.start,
            Mapping {
                id,
                length: range.end.value() - range.start.value(),
            },
        );

        // update LUT
        let start_page = range.start.value() >> PAGE_SHIFT;
        let end_page = range.end.value() >> PAGE_SHIFT;
        for index in start_page..=end_page {
            self.overlap_lut[index as usize] += 1;
        }
    }

    /// Returns the block starting at `addr`.
    #[inline(always)]
    pub fn get(&self, addr: Address) -> Option<BlockId> {
        if let Some((last_addr, id)) = self.last_query.get()
            && last_addr == addr
        {
            std::hint::cold_path();
            Some(id)
        } else {
            let id = self.tree_map.get(&addr)?.id;
            self.last_query.set(Some((addr, id)));
            Some(id)
        }
    }

    /// Returns the block starting at `addr`.
    #[inline(always)]
    pub fn get_uncached(&self, addr: Address) -> Option<BlockId> {
        if let Some((last_addr, id)) = self.last_query.get()
            && last_addr == addr
        {
            std::hint::cold_path();
            Some(id)
        } else {
            let id = self.tree_map.get(&addr)?.id;
            Some(id)
        }
    }

    pub fn clear(&mut self) {
        self.tree_map.clear();
        self.overlap_lut.fill(0);
        self.last_query.set(None);
    }
}

pub struct StoredBlock {
    pub block: Block,
    pub links: Vec<*mut LinkData>,
}

/// A structure which keeps tracks of compiled [`Block`]s.
pub struct Blocks {
    pub storage: SlotMap<BlockId, StoredBlock>,
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

        let id = self.storage.insert(StoredBlock {
            block,
            links: Vec::new(),
        });
        self.mapping.insert(range, id);

        id
    }

    #[inline(always)]
    pub fn get(&mut self, addr: Address) -> Option<&StoredBlock> {
        self.storage.get(self.mapping.get(addr)?)
    }

    #[inline(always)]
    pub fn get_uncached(&mut self, addr: Address) -> Option<&StoredBlock> {
        self.storage.get(self.mapping.get_uncached(addr)?)
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.storage.clear();
        self.mapping.clear();
    }

    /// Invalidates all blocks that contain `addr`.
    #[inline(always)]
    pub fn invalidate(&mut self, addr: Address) {
        // check LUT first
        let page = addr.value() >> PAGE_SHIFT;

        #[expect(clippy::redundant_else, reason = "makes it clearer")]
        if self.mapping.overlap_lut[page as usize] == 0 {
            return;
        } else {
            std::hint::cold_path();
        }

        self.mapping.to_remove.clear();
        for (&candidate, mapping) in self.mapping.tree_map.iter() {
            let length = mapping.length;
            let range = candidate..candidate + length;

            if range.contains(&addr) {
                self.mapping.to_remove.push(candidate);
            }
        }

        for target in self.mapping.to_remove.drain(..) {
            let mapping = self.mapping.tree_map.swap_remove(&target).unwrap();
            let block = self.storage.get_mut(mapping.id).unwrap();

            // invalidate links
            for link in block.links.drain(..) {
                let link = unsafe { link.as_mut().unwrap() };
                link.link = std::ptr::null();
            }

            // update LUT
            let start_page = target.value() >> PAGE_SHIFT;
            let end_page = (target + mapping.length).value() >> PAGE_SHIFT;
            for index in start_page..=end_page {
                self.mapping.overlap_lut[index as usize] -= 1;
            }
        }

        if self
            .mapping
            .last_query
            .get()
            .is_some_and(|(queried, _)| queried == addr)
        {
            self.mapping.last_query.set(None);
        }
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
    /// List of addresses that need to be invalidated.
    to_invalidate: &'a mut Vec<Address>,
    /// Amount of cycles we are trying to execute.
    target_cycles: u32,
    /// Maximum instructions we should execute.
    max_instructions: u32,
    /// Last followed link.
    last_followed_link: BlockFn,
    /// Reason for exit.
    exit_reason: ExitReason,
}

const CTX_HOOKS: Hooks = {
    extern "sysv64-unwind" fn get_registers<'a>(ctx: &'a mut Context) -> &'a mut Cpu {
        &mut ctx.sys.cpu
    }

    extern "sysv64-unwind" fn follow_link(
        info: &Info,
        ctx: &mut Context,
        link_data: &mut LinkData,
    ) -> bool {
        // if we have reached cycle or instruction limit, don't follow links, just exit.
        if info.cycles >= ctx.target_cycles || info.instructions >= ctx.max_instructions {
            ctx.last_followed_link = link_data.link;
            return false;
        }

        // otherwise, detect whether we are idle looping and exit too
        let follow = match link_data.pattern {
            Pattern::IdleBasic | Pattern::IdleVolatileRead => {
                if ctx.last_followed_link == link_data.link {
                    ctx.exit_reason = ExitReason::IdleLooping;
                    false
                } else {
                    true
                }
            }
            _ => true,
        };

        // if not idle looping, then sure, follow link
        ctx.last_followed_link = link_data.link;
        follow
    }

    extern "sysv64-unwind" fn try_link(ctx: &mut Context, addr: Address, link_data: &mut LinkData) {
        debug_assert!(link_data.link.is_null());
        if let Some(id) = ctx.blocks.mapping.get(addr) {
            let stored = ctx.blocks.storage.get_mut(id).unwrap();
            link_data.link = stored.block.as_ptr();
            link_data.pattern = stored.block.meta().pattern;
            stored.links.push(&raw mut *link_data);
        }
    }

    extern "sysv64-unwind" fn read<P: Primitive>(
        ctx: &mut Context,
        addr: Address,
        value: &mut P,
    ) -> bool {
        if let Some(physical) = ctx.sys.translate_data_addr(addr) {
            *value = ctx.sys.read(physical);
            // tracing::debug!(
            //     "reading from logical {addr}, physical {physical}: 0x{:X?}",
            //     value
            // );
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
        let Some(physical) = ctx.sys.translate_data_addr(addr) else {
            std::hint::cold_path();
            tracing::error!(pc = ?ctx.sys.cpu.pc, "failed to translate address {addr}");
            return false;
        };

        // tracing::debug!(
        //     "writing to logical {addr}, physical {physical}: 0x{:X?}",
        //     value
        // );

        ctx.sys.write(physical, value);
        ctx.to_invalidate.push(addr);

        seq! {
            N in 1..4 {
                if const { size_of::<P>() >= N } {
                    ctx.to_invalidate.push(addr + N);
                }
            }
        }

        true
    }

    extern "sysv64-unwind" fn read_quantized(
        ctx: &mut Context,
        addr: Address,
        gqr: u8,
        value: &mut f64,
    ) -> u8 {
        let Some(physical) = ctx.sys.translate_data_addr(addr) else {
            std::hint::cold_path();
            tracing::error!("failed to translate address {addr}");
            return 0;
        };

        let gqr = ctx.sys.cpu.supervisor.gq[gqr as usize].clone();
        let scale = if gqr.load_type() != QuantizedType::Float {
            gqr.load_scale().value()
        } else {
            0
        };

        let read = match gqr.load_type() {
            QuantizedType::U8 => ctx.sys.read::<u8>(physical) as f64,
            QuantizedType::U16 => ctx.sys.read::<u16>(physical) as f64,
            QuantizedType::I8 => ctx.sys.read::<i8>(physical) as f64,
            QuantizedType::I16 => ctx.sys.read::<i16>(physical) as f64,
            _ => f32::from_bits(ctx.sys.read::<u32>(physical)) as f64,
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
        let Some(physical) = ctx.sys.translate_data_addr(addr) else {
            std::hint::cold_path();
            tracing::error!("failed to translate address {addr}");
            return 0;
        };

        let gqr = ctx.sys.cpu.supervisor.gq[gqr as usize].clone();
        let scale = if gqr.store_type() != QuantizedType::Float {
            gqr.store_scale().value()
        } else {
            0
        };
        let scaled = value * 2.0f64.powi(-scale as i32);

        match gqr.store_type() {
            QuantizedType::U8 => ctx.sys.write(physical, scaled as u8),
            QuantizedType::U16 => ctx.sys.write(physical, scaled as u16),
            QuantizedType::I8 => ctx.sys.write(physical, scaled as i8),
            QuantizedType::I16 => ctx.sys.write(physical, scaled as i16),
            _ => ctx.sys.write(physical, (scaled as f32).to_bits()),
        }

        gqr.store_type().size()
    }

    extern "sysv64-unwind" fn cache_dma(ctx: &mut Context) {
        let dma = ctx.sys.cpu.supervisor.config.dma.clone();

        if dma.lower.trigger() {
            let ram =
                &mut ctx.sys.mem.ram[dma.mem_address().value() as usize..][..dma.length() as usize];
            let l2c = &mut ctx.sys.mem.l2c[dma.cache_address().value() as usize - 0xE000_0000..]
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
        ctx.blocks.mapping.clear();
        ctx.sys
            .mmu
            .build_instr_bat_lut(&ctx.sys.cpu.supervisor.memory.ibat);
    }

    extern "sysv64-unwind" fn dbat_changed(ctx: &mut Context) {
        tracing::info!("dbats changed - rebuilding dbat lut");
        ctx.sys
            .mmu
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
        let cache_dma = transmute::<_, GenericHook>(cache_dma as extern "sysv64-unwind" fn(_));

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
            cache_dma,

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

impl Default for Config {
    fn default() -> Self {
        Self {
            instr_per_block: 256,
            jit_settings: Default::default(),
        }
    }
}

pub struct JitCore {
    pub config: Config,
    pub compiler: ppcjit::JIT,
    pub trampoline: Trampoline,
    pub blocks: Blocks,

    to_invalidate: Vec<Address>,
}

impl JitCore {
    pub fn new(config: Config) -> Self {
        let mut compiler = ppcjit::JIT::new(config.jit_settings.clone(), CTX_HOOKS);
        let trampoline = compiler.trampoline();

        Self {
            config,
            compiler,
            trampoline,
            blocks: Blocks::default(),

            to_invalidate: Vec::with_capacity(16),
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

            let ins = Ins::new(sys.read(physical), Extensions::gekko_broadway());
            count += 1;

            Some(ins)
        });

        let block = match self.compiler.compile(instructions) {
            Ok(b) => b,
            Err(e) => match &e {
                ppcjit::BuildError::EmptyBlock => panic!("built empty block at pc {}", sys.cpu.pc),
                ppcjit::BuildError::Builder { .. } => panic!("block builder error: {}", e),
                ppcjit::BuildError::Codegen { .. } => panic!("block codegen error: {}", e),
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
            .mapping
            .get(sys.cpu.pc)
            .and_then(|id| self.blocks.storage.get(id))
            .filter(|b| b.block.meta().seq.len() <= max_instructions as usize);

        let compiled: ppcjit::Block;
        let block = match stored {
            Some(stored) => stored.block.as_ptr(),
            None => {
                std::hint::cold_path();

                compiled = self.compile(sys, sys.cpu.pc, max_instructions);
                compiled.as_ptr()
            }
        };

        let mut ctx = Context {
            sys,
            blocks: &mut self.blocks,
            to_invalidate: &mut self.to_invalidate,
            target_cycles,
            max_instructions,

            last_followed_link: std::ptr::null(),
            exit_reason: ExitReason::None,
        };

        let info = unsafe {
            self.trampoline
                .call(&raw mut ctx as *mut ppcjit::hooks::Context, block)
        };

        let cycles = if ctx.exit_reason == ExitReason::IdleLooping {
            std::hint::cold_path();
            Cycles(target_cycles as u64)
        } else {
            Cycles(info.cycles as u64)
        };

        if !self.to_invalidate.is_empty() {
            std::hint::cold_path();
            for addr in self.to_invalidate.drain(..) {
                self.blocks.invalidate(addr);
            }
        }

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
            .mapping
            .get(sys.cpu.pc)
            .and_then(|id| self.blocks.storage.get(id))
            .filter(|b| b.block.meta().seq.len() <= max_instructions as usize);

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
                && stored.block.meta().pattern == Pattern::Call
                && let Some(dest) = stored.block.meta().seq.is_call(sys.cpu.pc)
            {
                std::hint::cold_path();

                if let Some(func_block) = self.blocks.get_uncached(dest)
                    && func_block.block.meta().pattern == Pattern::GetMailboxStatusFunc
                    && sys.dsp.cpu_mailbox.status()
                {
                    std::hint::cold_path();
                    executed.cycles = cycles;
                    executed.instructions = 1;
                    break;
                }
            }

            let max_instructions = if BREAKPOINTS {
                // find closest breakpoint
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

impl CpuCore for JitCore {
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
