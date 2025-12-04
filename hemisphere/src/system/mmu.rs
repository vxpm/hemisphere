///! Memory mapping.
use bitos::BitUtils;
use gekko::{Address, Bat, MemoryManagement};
use tracing::{info_span, trace, warn};

const BASES_COUNT: usize = 1 << 15;
type BlockLUT = Box<[u16; BASES_COUNT]>;

const NO_BAT: u16 = 1;

/// Struct that performs efficient memory mapping
pub struct Mmu {
    data_bat_lut: BlockLUT,
    instr_bat_lut: BlockLUT,
}

impl Default for Mmu {
    fn default() -> Self {
        Self {
            data_bat_lut: util::boxed_array(NO_BAT),
            instr_bat_lut: util::boxed_array(NO_BAT),
        }
    }
}

impl Mmu {
    fn update_lut_with(lut: &mut BlockLUT, bat: &Bat) {
        let physical_start_base = (bat.physical_start().value() >> 17) as u16;
        let physical_end_base = (bat.physical_end().value() >> 17) as u16;
        let logical_start_base = bat.start().value() >> 17;
        let logical_end_base = bat.end().value() >> 17;

        trace!(
            "start = {}, end = {}, physical start = {}, physical end = {}",
            bat.start(),
            bat.end(),
            bat.physical_start(),
            bat.physical_end()
        );
        trace!(
            "start base = {:04X}, end base = {:04X}, physical start base = {:04X}, physical end base = {:04X}",
            logical_start_base, logical_end_base, physical_start_base, physical_end_base
        );

        for (i, base) in (logical_start_base..=logical_end_base).enumerate() {
            lut[base as usize] = (physical_start_base + i as u16) << 1;
        }
    }

    pub fn build_data_bat_lut(&mut self, dbats: &[Bat; 4]) {
        let _span = info_span!("building dbat lut").entered();

        self.data_bat_lut.fill(NO_BAT);
        for (i, bat) in dbats.iter().enumerate() {
            if !bat.supervisor_mode() {
                warn!("dbat{i} is disabled in supervisor mode");
                continue;
            }

            Self::update_lut_with(&mut self.data_bat_lut, bat);
        }
    }

    pub fn build_instr_bat_lut(&mut self, ibats: &[Bat; 4]) {
        let _span = info_span!("building ibat lut").entered();

        self.instr_bat_lut.fill(NO_BAT);
        for (i, bat) in ibats.iter().enumerate() {
            if !bat.supervisor_mode() {
                warn!("ibat{i} is disabled in supervisor mode");
                continue;
            }

            Self::update_lut_with(&mut self.instr_bat_lut, bat);
        }
    }

    pub fn build_bat_lut(&mut self, memory: &MemoryManagement) {
        let _span = info_span!("building bat luts").entered();

        self.build_data_bat_lut(&memory.dbat);
        self.build_instr_bat_lut(&memory.ibat);
    }

    pub fn translate_data_addr<A: Into<Address>>(&self, addr: A) -> Option<A>
    where
        Address: Into<A>,
    {
        let addr = addr.into().value();
        let logical_base = addr >> 17;
        let physical_base = self.data_bat_lut[logical_base as usize] as u32;

        if physical_base == NO_BAT as u32 {
            std::hint::cold_path();
            None
        } else {
            let base = physical_base << 16;
            Some(Address(base | addr.bits(0, 17)).into())
        }
    }

    pub fn translate_instr_addr(&self, addr: Address) -> Option<Address> {
        let addr = addr.value();
        let logical_base = addr >> 17;
        let physical_base = self.instr_bat_lut[logical_base as usize] as u32;

        if physical_base == NO_BAT as u32 {
            std::hint::cold_path();
            None
        } else {
            let base = physical_base << 16;
            Some(Address(base | addr.bits(0, 17)))
        }
    }
}
