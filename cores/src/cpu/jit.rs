use hemisphere::{
    Address, Cycles, Primitive,
    cores::{CpuCore, Executed},
    gekko::{
        self, Cpu, QuantizedType,
        disasm::{Extensions, Ins},
    },
    system::System,
};
use ppcjit::{
    Block,
    block::{
        GenericHook, GetRegistersHook, Hooks, IdleLoop, ReadHook, ReadQuantizedHook, WriteHook,
        WriteQuantizedHook,
    },
};
use slotmap::{SlotMap, new_key_type};
use std::{collections::BTreeMap, ops::Range};

pub use ppcjit;

/// A page is 4096 (1^12) bytes. Therefore, there are 2^20 pages.
const PAGE_COUNT: usize = 1 << 20;
type PageLut = Box<[u16; PAGE_COUNT]>;

new_key_type! {
    /// Identifier for a block in a [`Blocks`] storage.
    pub struct BlockId;
}

/// Mapping of addresses to JIT blocks.
pub struct BlockMapping {
    address: BTreeMap<Address, (BlockId, u32)>,
    page_lut: PageLut,

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
            page_lut: util::boxed_array(0),

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
struct Context<'a> {
    /// The system state, so that the JIT block can operate on it.
    system: &'a mut System,
    /// The block mapping, so that write operations can invalidate blocks.
    mapping: &'a mut BlockMapping,
}

static CTX_HOOKS: Hooks = {
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

    extern "sysv64-unwind" fn cache_dma(ctx: &mut Context) {
        let dma = ctx.system.cpu.supervisor.config.dma.clone();

        if dma.lower.trigger() {
            let ram = &mut ctx.system.mem.ram[dma.mem_address().value() as usize..]
                [..dma.length() as usize];
            let l2c = &mut ctx.system.mem.l2c[dma.cache_address().value() as usize - 0xE000_0000..]
                [..dma.length() as usize];

            match dma.lower.direction() {
                gekko::DmaDirection::FromCacheToRam => {
                    ram.copy_from_slice(l2c);
                }
                gekko::DmaDirection::FromRamToCache => {
                    l2c.copy_from_slice(ram);
                }
            }
        }

        ctx.system
            .cpu
            .supervisor
            .config
            .dma
            .lower
            .set_trigger(false)
            .set_flush(false);
    }

    extern "sysv64-unwind" fn msr_changed(ctx: &mut Context) {
        ctx.system
            .scheduler
            .schedule_now(System::pi_check_interrupts);
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
        ctx.system.scheduler.cancel(System::decrementer_overflow);

        let dec = ctx.system.cpu.supervisor.misc.dec;
        tracing::trace!("decrementer changed to {dec}");

        ctx.system
            .scheduler
            .schedule(dec as u64, System::decrementer_overflow);
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
    pub compiler: ppcjit::Compiler,
    pub blocks: Blocks,
}

impl JitCore {
    pub fn new(config: Config) -> Self {
        Self {
            compiler: ppcjit::Compiler::new(config.jit_settings.clone()),
            blocks: Blocks::default(),
            config,
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
    fn uncached_exec(&mut self, sys: &mut System, max_instructions: u32) -> Executed {
        let block = self
            .blocks
            .mapping
            .get(sys.cpu.pc)
            .and_then(|id| self.blocks.storage.get(id))
            .filter(|b| b.meta().seq.len() <= max_instructions as usize);

        let compiled: ppcjit::Block;
        let block = match block {
            Some(block) => block,
            None => {
                std::hint::cold_path();

                compiled = self.compile(sys, sys.cpu.pc, max_instructions);
                &compiled
            }
        };

        let mut ctx = Context {
            system: sys,
            mapping: &mut self.blocks.mapping,
        };

        let executed = block.call(&raw mut ctx as *mut ppcjit::block::Context, &CTX_HOOKS);
        Executed {
            instructions: executed.instructions,
            cycles: Cycles(executed.cycles as u64),
            hit_breakpoint: false,
        }
    }

    fn cached_exec(&mut self, sys: &mut System, max_instructions: u32) -> Executed {
        let block = self
            .blocks
            .mapping
            .get(sys.cpu.pc)
            .and_then(|id| self.blocks.storage.get(id))
            .filter(|b| b.meta().seq.len() <= max_instructions as usize);

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

        self.uncached_exec(sys, max_instructions)
    }

    #[inline(always)]
    fn detect_idle_loop(&mut self, sys: &System) -> IdleLoop {
        let block = self
            .blocks
            .mapping
            .get(sys.cpu.pc)
            .and_then(|id| self.blocks.storage.get(id));

        block.map(|b| b.meta().idle_loop).unwrap_or(IdleLoop::None)
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
        let mut executed = Executed::default();
        let mut volatile_idle_loop = None;
        while executed.cycles < cycles {
            match self.detect_idle_loop(sys) {
                IdleLoop::None => (),
                IdleLoop::Simple => {
                    std::hint::cold_path();
                    executed.instructions += 1;
                    executed.cycles = cycles;
                    break;
                }
                IdleLoop::VolatileValue => {
                    std::hint::cold_path();
                    match volatile_idle_loop {
                        Some(start) if start == sys.cpu.pc => {
                            executed.cycles = cycles;
                            break;
                        }
                        None | Some(_) => volatile_idle_loop = Some(sys.cpu.pc),
                    }
                }
            }

            // find closest breakpoint
            let closest_breakpoint = closest_breakpoint(sys.cpu.pc, breakpoints);
            let breakpoint_distance = (closest_breakpoint.value() - sys.cpu.pc.value()) / 4;

            // execute
            let e = self.cached_exec(sys, breakpoint_distance);
            executed.instructions += e.instructions;
            executed.cycles += e.cycles;

            if breakpoints.contains(&sys.cpu.pc) {
                executed.hit_breakpoint = true;
                break;
            }
        }

        executed
    }

    fn step(&mut self, sys: &mut System) -> Executed {
        self.uncached_exec(sys, 1)
    }
}
