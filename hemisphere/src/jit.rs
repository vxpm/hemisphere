use crate::bus::Bus;
use hemicore::{Address, Primitive, arch::Registers};
use ppcjit::{Block, block::ExternalFunctions};
use rustc_hash::FxHashMap;
use schnellru::{ByLength, LruMap};
use std::ops::Range;

struct StoredBlock {
    /// Address this block was built for.
    addr: Address,
    /// The block.
    block: Block,
}

impl StoredBlock {
    fn start(&self) -> Address {
        self.addr
    }

    fn end(&self) -> Address {
        self.addr + self.block.sequence().len() as u32 * 4
    }

    fn range(&self) -> Range<Address> {
        (self.start())..(self.end())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
struct BlockId(usize);

/// A structure which keeps tracks of compiled [`Block`]s.
pub struct BlockStorage {
    current: usize,
    blocks: LruMap<BlockId, StoredBlock, ByLength>,
    map: FxHashMap<Address, BlockId>,
    buffer: Vec<BlockId>,
}

impl Default for BlockStorage {
    fn default() -> Self {
        Self {
            current: 0,
            blocks: LruMap::new(ByLength::new(1024)),
            map: FxHashMap::default(),
            buffer: Vec::new(),
        }
    }
}

impl BlockStorage {
    pub fn insert(&mut self, addr: Address, block: Block) -> &Block {
        let id = BlockId(self.current);
        self.current += 1;
        self.blocks.insert(id, StoredBlock { addr, block });
        self.map.insert(addr, id);

        &self.blocks.get(&id).unwrap().block
    }

    pub fn get(&mut self, addr: Address) -> Option<&Block> {
        self.map
            .get(&addr)
            .and_then(|id| self.blocks.get(id))
            .map(|b| &b.block)
    }

    pub fn invalidate(&mut self, addresses: &[Address]) {
        self.buffer.clear();
        for (id, block) in self.blocks.iter() {
            for address in addresses {
                if block.range().contains(address) {
                    self.buffer.push(*id);
                }
            }
        }
    }
}

/// External data to be passed in for execution of JIT blocks.
pub struct ExternalData<'a> {
    pub bus: &'a mut Bus,
    pub invalidated: &'a mut Vec<Address>,
}

impl<'a> ExternalData<'a> {
    pub fn functions() -> ExternalFunctions {
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

        let read_i8 =
            unsafe { std::mem::transmute(read::<i8> as extern "sysv64" fn(_, _, _) -> _) };
        let write_i8 =
            unsafe { std::mem::transmute(write::<i8> as extern "sysv64" fn(_, _, _, _)) };
        let read_i16 =
            unsafe { std::mem::transmute(read::<i16> as extern "sysv64" fn(_, _, _) -> _) };
        let write_i16 =
            unsafe { std::mem::transmute(write::<i16> as extern "sysv64" fn(_, _, _, _)) };
        let read_i32 =
            unsafe { std::mem::transmute(read::<i32> as extern "sysv64" fn(_, _, _) -> _) };
        let write_i32 =
            unsafe { std::mem::transmute(write::<i32> as extern "sysv64" fn(_, _, _, _)) };

        ExternalFunctions {
            read_i8,
            write_i8,
            read_i16,
            write_i16,
            read_i32,
            write_i32,
        }
    }
}

/// The JIT context.
pub struct JIT {
    pub compiler: ppcjit::Compiler,
    pub blocks: BlockStorage,
}

impl JIT {
    pub fn new() -> Self {
        Self {
            compiler: ppcjit::Compiler::default(),
            blocks: BlockStorage::default(),
        }
    }
}
