use lazuli::Address;

use crate::cpu::jit::BlockId;
use crate::cpu::jit::table::Table as BaseTable;

const MAP_TBL_L0_BITS: usize = 12;
const MAP_TBL_L0_COUNT: usize = 1 << MAP_TBL_L0_BITS;
const MAP_TBL_L0_MASK: usize = MAP_TBL_L0_COUNT - 1;
const MAP_TBL_L1_BITS: usize = 8;
const MAP_TBL_L1_COUNT: usize = 1 << MAP_TBL_L1_BITS;
const MAP_TBL_L1_MASK: usize = MAP_TBL_L1_COUNT - 1;
const MAP_TBL_L2_BITS: usize = 10;
const MAP_TBL_L2_COUNT: usize = 1 << MAP_TBL_L2_BITS;
const MAP_TBL_L2_MASK: usize = MAP_TBL_L2_COUNT - 1;

#[inline(always)]
fn addr_to_mapping_idx(addr: Address) -> (usize, usize, usize) {
    let base = (addr.value() >> 2) as usize;
    (
        base >> (30 - MAP_TBL_L0_BITS) & MAP_TBL_L0_MASK,
        (base >> (30 - MAP_TBL_L0_BITS - MAP_TBL_L1_BITS)) & MAP_TBL_L1_MASK,
        (base >> (30 - MAP_TBL_L0_BITS - MAP_TBL_L1_BITS - MAP_TBL_L2_BITS)) & MAP_TBL_L2_MASK,
    )
}

#[derive(Debug, Clone, Copy)]
pub struct Mapping {
    pub id: BlockId,
    pub length: u32,
}

#[derive(Default)]
pub struct Table(
    BaseTable<BaseTable<BaseTable<Mapping, MAP_TBL_L2_COUNT>, MAP_TBL_L1_COUNT>, MAP_TBL_L0_COUNT>,
);

impl Table {
    #[inline(always)]
    pub fn insert(&mut self, addr: Address, mapping: Mapping) {
        let (idx0, idx1, idx2) = addr_to_mapping_idx(addr);
        let level1 = self.0.get_or_default(idx0);
        let level2 = level1.get_or_default(idx1);
        level2.insert(idx2, mapping);
    }

    #[inline(always)]
    pub fn remove(&mut self, addr: Address) -> Option<Mapping> {
        let (idx0, idx1, idx2) = addr_to_mapping_idx(addr);
        let level1 = self.0.get_mut(idx0)?;
        let level2 = level1.get_mut(idx1)?;
        level2.remove(idx2)
    }

    #[inline(always)]
    pub fn get(&self, addr: Address) -> Option<&Mapping> {
        let (idx0, idx1, idx2) = addr_to_mapping_idx(addr);
        let level1 = self.0.get(idx0)?;
        let level2 = level1.get(idx1)?;
        level2.get(idx2)
    }
}
