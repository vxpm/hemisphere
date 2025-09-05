use crate::bus::Bus;
use hemicore::{Address, Primitive, arch::Registers, util::boxed_array};
use interavl::IntervalTree;
use ppcjit::{
    Block,
    block::{ExternalFunctions, ReadFunction, WriteFunction},
};
use slotmap::{SlotMap, new_key_type};
use std::{collections::BTreeMap, ops::Range};

const PAGE_COUNT: usize = 2usize.pow(20);

new_key_type! {
    pub struct BlockId;
}

/// A structure which keeps tracks of compiled [`Block`]s.
pub struct BlockStorage {
    blocks: SlotMap<BlockId, Block>,
    mapping: BTreeMap<Address, BlockId>,
    intervals: IntervalTree<u64, BlockId>,
    page_lut: Box<[u16; PAGE_COUNT]>,

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
    pub fn insert(&mut self, addr: Address, block: Block) -> &Block {
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

        self.blocks.get(id).unwrap()
    }

    /// Returns the block starting at `addr`.
    #[inline(always)]
    pub fn get(&mut self, addr: Address) -> Option<&Block> {
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
        unsafe { Some(self.blocks.get_unchecked(id)) }
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
}

/// External data to be passed in for execution of JIT blocks.
pub struct ExternalData<'a> {
    pub bus: &'a mut Bus,
    pub invalidated: &'a mut Vec<Address>,
}

pub static EXTERNAL_FUNCTIONS: ExternalFunctions = {
    extern "sysv64" fn read<T: Primitive>(
        external: &mut ExternalData,
        registers: &Registers,
        addr: Address,
    ) -> T {
        let physical = registers.supervisor.translate_data_addr(addr);
        external.bus.read(physical)
    }

    extern "sysv64" fn write<T: Primitive>(
        external: &mut ExternalData,
        registers: &Registers,
        addr: Address,
        value: T,
    ) {
        external.invalidated.push(addr);

        let physical = registers.supervisor.translate_data_addr(addr);
        external.bus.write(physical, value);
    }

    extern "sysv64" fn bat_changed(external: &mut ExternalData) {
        // rebuild bat LUT

        // rebuild block page LUT
    }

    #[expect(
        clippy::missing_transmute_annotations,
        reason = "unnecessary - the definitions are above"
    )]
    unsafe {
        use std::mem::transmute;
        let read_i8 =
            transmute::<_, ReadFunction<i8>>(read::<i8> as extern "sysv64" fn(_, _, _) -> _);
        let write_i8 =
            transmute::<_, WriteFunction<i8>>(write::<i8> as extern "sysv64" fn(_, _, _, _));
        let read_i16 =
            transmute::<_, ReadFunction<i16>>(read::<i16> as extern "sysv64" fn(_, _, _) -> _);
        let write_i16 =
            transmute::<_, WriteFunction<i16>>(write::<i16> as extern "sysv64" fn(_, _, _, _));
        let read_i32 =
            transmute::<_, ReadFunction<i32>>(read::<i32> as extern "sysv64" fn(_, _, _) -> _);
        let write_i32 =
            transmute::<_, WriteFunction<i32>>(write::<i32> as extern "sysv64" fn(_, _, _, _));

        ExternalFunctions {
            read_i8,
            write_i8,
            read_i16,
            write_i16,
            read_i32,
            write_i32,
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
