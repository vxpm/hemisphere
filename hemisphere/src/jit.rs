use easyerr::Error;
use hemicore::Address;
use ppcjit::block::Block;
use std::collections::{HashMap, hash_map::Entry};

/// A structure which keeps tracks of compiled [`Block`]s.
#[derive(Default)]
pub struct BlockStorage {
    regions: HashMap<u32, HashMap<Address, Block>>,
}

pub fn region(addr: Address) -> u32 {
    // 256 bytes regions (64 instructions)
    addr.value() >> 8
}

#[derive(Debug, Error)]
#[error("a block already exists at the given address")]
pub struct InsertError;

impl BlockStorage {
    pub fn insert(&mut self, addr: Address, block: Block) -> Result<&Block, InsertError> {
        let region = region(addr);
        let region = self.regions.entry(region).or_default();
        match region.entry(addr) {
            Entry::Occupied(_) => return Err(InsertError),
            Entry::Vacant(v) => Ok(v.insert(block)),
        }
    }

    pub fn remove(&mut self, addr: Address) -> Option<Block> {
        let region = region(addr);
        let region = self.regions.get_mut(&region)?;

        region.remove(&addr)
    }

    pub fn get(&self, addr: Address) -> Option<&Block> {
        let region = region(addr);
        let region = self.regions.get(&region)?;

        region.get(&addr)
    }

    pub fn entry(&mut self, addr: Address) -> Entry<'_, Address, Block> {
        let region = region(addr);
        let region = self.regions.entry(region).or_default();

        region.entry(addr)
    }

    pub fn region(&mut self, region: u32) -> Option<&HashMap<Address, Block>> {
        self.regions.get(&region)
    }

    pub fn clear(&mut self) {
        self.regions.clear();
    }
}
