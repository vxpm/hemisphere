use crate::System;
use hemicore::{Address, Primitive, arch::Registers, util::boxed_array};
use interavl::IntervalTree;
use ppcjit::{
    Block,
    block::{ExternalFunctions, GenericHookFn, GetRegistersFn, ReadFn, WriteFn},
};
use slotmap::{SlotMap, new_key_type};
use std::{collections::BTreeMap, ops::Range};

const PAGE_COUNT: usize = 1 << 20;
type PageLUT = Box<[u16; PAGE_COUNT]>;

new_key_type! {
    pub struct BlockId;
}

/// A structure which keeps tracks of compiled [`Block`]s.
pub struct BlockStorage {
    blocks: SlotMap<BlockId, Block>,
    mapping: BTreeMap<Address, BlockId>,
    intervals: IntervalTree<u64, BlockId>,
    page_lut: PageLUT,

    // caching stuff
    buffer: Vec<(Range<u64>, BlockId)>,
    last_query: Option<(Address, BlockId)>,
}

impl Default for BlockStorage {
    fn default() -> Self {
        Self {
            blocks: SlotMap::with_key(),
            mapping: BTreeMap::default(),
            intervals: IntervalTree::default(),
            page_lut: boxed_array(0),

            buffer: Vec::with_capacity(64),
            last_query: None,
        }
    }
}

impl BlockStorage {
    /// Inserts a new block in the storage.
    pub fn insert(&mut self, addr: Address, block: Block) {
        let start = addr.value() as u64;
        let end = start + block.sequence().len() as u64 * 4;
        let range = start..end;

        let id = self.blocks.insert(block);
        self.mapping.insert(addr, id);
        self.intervals.insert(range, id);

        // update LUT
        let start_page = start >> 12;
        let end_page = end >> 12;
        for index in start_page..=end_page {
            self.page_lut[index as usize] += 1;
        }
    }

    /// Returns the block starting at `addr`.
    #[inline(always)]
    pub fn get(&mut self, addr: Address) -> Option<Block> {
        let id = if let Some((last_addr, id)) = self.last_query
            && last_addr == addr
        {
            std::hint::cold_path();
            id
        } else {
            let id = *self.mapping.get(&addr)?;
            self.last_query = Some((addr, id));
            id
        };

        // SAFETY: if the block exists in the mapping, it should exist in the blocks!
        unsafe { Some(self.blocks.get_unchecked(id).clone()) }
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

        let start = addr.value() as u64;
        let range = start..start + 1;

        for (range, id) in self.intervals.iter_overlaps(&range) {
            self.buffer.push((range.clone(), *id));
        }

        for (range, id) in self.buffer.drain(..) {
            self.blocks.remove(id);
            self.mapping.remove(&Address(range.start as u32));
            self.intervals.remove(&range);

            // update LUT
            let start_page = range.start >> 12;
            let end_page = range.end >> 12;
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
        self.blocks.clear();
        self.mapping.clear();
        self.intervals = IntervalTree::default();
        self.page_lut.fill(0);

        self.buffer.clear();
        self.last_query = None;
    }
}

/// External data to be passed in for execution of JIT blocks.
pub struct ExternalData<'a> {
    pub system: &'a mut System,
    pub blocks: &'a mut BlockStorage,
}

pub static EXTERNAL_FUNCTIONS: ExternalFunctions = {
    extern "sysv64" fn get_registers<'a>(external: &'a mut ExternalData) -> &'a mut Registers {
        &mut external.system.cpu
    }

    extern "sysv64" fn read<T: Primitive>(external: &mut ExternalData, addr: Address) -> T {
        let physical = external.system.translate_data_addr(addr);
        external.system.bus.read(physical)
    }

    extern "sysv64" fn write<T: Primitive>(external: &mut ExternalData, addr: Address, value: T) {
        let physical = external.system.translate_data_addr(addr);
        external.system.bus.write(physical, value);
        external.blocks.invalidate(addr);
    }

    extern "sysv64" fn ibat_changed(external: &mut ExternalData) {
        external.blocks.clear();
        external
            .system
            .mmu
            .build_bat_lut(&external.system.cpu.supervisor.memory);
    }

    extern "sysv64" fn dbat_changed(external: &mut ExternalData) {
        external
            .system
            .mmu
            .build_bat_lut(&external.system.cpu.supervisor.memory);
    }

    #[expect(
        clippy::missing_transmute_annotations,
        reason = "unnecessary - the definitions are above"
    )]
    unsafe {
        use std::mem::transmute;

        let get_registers =
            transmute::<_, GetRegistersFn>(get_registers as extern "sysv64" fn(_) -> _);

        let read_i8 = transmute::<_, ReadFn<i8>>(read::<i8> as extern "sysv64" fn(_, _) -> _);
        let write_i8 = transmute::<_, WriteFn<i8>>(write::<i8> as extern "sysv64" fn(_, _, _));
        let read_i16 = transmute::<_, ReadFn<i16>>(read::<i16> as extern "sysv64" fn(_, _) -> _);
        let write_i16 = transmute::<_, WriteFn<i16>>(write::<i16> as extern "sysv64" fn(_, _, _));
        let read_i32 = transmute::<_, ReadFn<i32>>(read::<i32> as extern "sysv64" fn(_, _) -> _);
        let write_i32 = transmute::<_, WriteFn<i32>>(write::<i32> as extern "sysv64" fn(_, _, _));

        let ibat_changed = transmute::<_, GenericHookFn>(ibat_changed as extern "sysv64" fn(_));
        let dbat_changed = transmute::<_, GenericHookFn>(dbat_changed as extern "sysv64" fn(_));

        ExternalFunctions {
            get_registers,

            read_i8,
            write_i8,
            read_i16,
            write_i16,
            read_i32,
            write_i32,

            ibat_changed,
            dbat_changed,
        }
    }
};

/// The JIT context.
pub struct JIT {
    pub compiler: ppcjit::Compiler,
    pub blocks: BlockStorage,
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
            blocks: BlockStorage::default(),
        }
    }
}
