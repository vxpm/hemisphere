use bimap::BiHashMap;
use easyerr::Error;
use hemicore::Address;
use ppcjit::block::Block;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use slotmap::{SlotMap, new_key_type};

new_key_type! {
    /// The ID of a JIT block in a [`BlockStorage`].
    pub struct BlockId;
}

/// A structure which keeps tracks of compiled [`Block`]s.
///
/// Every block is associated with a single address, and vice-versa. However, blocks span multiple
/// addresses and therefore must be invalidated on writes to any of them - not only the starting
/// address.
///
/// In order to keep track of this, [`BlockStorage`] keeps track of which blocks every 256 bytes
/// region of memory is "touched" by. Then, whenever an address inside a region is invalidated, all
/// blocks that touch that region get invalidated.
#[derive(Default)]
pub struct BlockStorage {
    blocks: SlotMap<BlockId, Block>,
    mapping: BiHashMap<Address, BlockId, FxBuildHasher, FxBuildHasher>,
    regions: FxHashMap<u32, FxHashSet<BlockId>>,
}

pub fn region(addr: Address) -> u32 {
    // 256 bytes regions (64 instructions)
    addr.value() >> 8
}

#[derive(Debug, Error)]
#[error("a block already exists at the given address")]
pub struct InsertError;

impl BlockStorage {
    pub fn insert(&mut self, addr: Address, block: Block) -> Result<BlockId, InsertError> {
        if let Some(id) = self.mapping.get_by_left(&addr)
            && self.blocks.contains_key(*id)
        {
            return Err(InsertError);
        }

        let start_region = region(addr);
        let end_region = region(addr + block.sequence().len() as u32 * 4);

        let id = self.blocks.insert(block);
        self.mapping.insert(addr, id);

        for region in start_region..=end_region {
            let region = self.regions.entry(region).or_default();
            region.insert(id);
        }

        Ok(id)
    }

    pub fn remove(&mut self, id: BlockId) -> Option<Block> {
        let block = self.blocks.remove(id)?;
        let (addr, _) = self
            .mapping
            .remove_by_right(&id)
            .expect("block exists, so the mapping also does");

        let start_region = region(addr);
        let end_region = region(addr + block.sequence().len() as u32 * 4);

        for region in start_region..=end_region {
            let region = self.regions.get_mut(&region).expect("region should exist");
            region.remove(&id);
        }

        Some(block)
    }

    pub fn get_by_id(&self, id: BlockId) -> Option<&Block> {
        self.blocks.get(id)
    }

    pub fn get(&self, addr: Address) -> Option<&Block> {
        let id = self.mapping.get_by_left(&addr)?;
        self.blocks.get(*id)
    }

    pub fn invalidate(&mut self, addr: Address) {
        let region = region(addr);
        let Some(region) = self.regions.get_mut(&region) else {
            return;
        };

        for id in region.drain() {
            self.blocks.remove(id);
            self.mapping.remove_by_right(&id);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Address, &Block)> {
        self.mapping
            .iter()
            .map(|(addr, id)| (*addr, &self.blocks[*id]))
    }

    pub fn clear(&mut self) {
        self.regions.clear();
    }
}
