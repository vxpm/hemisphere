use bitos::BitUtils;
use hemicore::{
    Address,
    arch::{Bat, MemoryManagement},
    util::boxed_array,
};
use tracing::{debug, debug_span};

const BASES_COUNT: usize = 1 << 15;
type BATLUT = Box<[u16; BASES_COUNT]>;

const NO_BAT: u16 = 1;

pub struct Mmu {
    data_bat_lut: BATLUT,
    instr_bat_lut: BATLUT,
}

impl Mmu {
    pub fn new() -> Self {
        Self {
            data_bat_lut: boxed_array(NO_BAT),
            instr_bat_lut: boxed_array(NO_BAT),
        }
    }

    pub fn build_bat_lut(&mut self, mem: &MemoryManagement) {
        let _span = debug_span!("building bat lut");

        self.data_bat_lut.fill(NO_BAT);
        self.instr_bat_lut.fill(NO_BAT);

        fn inner(lut: &mut BATLUT, bat: &Bat) {
            let physical_start_base = (bat.physical_start().value() >> 17) as u16;
            let physical_end_base = (bat.physical_end().value() >> 17) as u16;
            let logical_start_base = bat.start().value() >> 17;
            let logical_end_base = bat.end().value() >> 17;

            debug!(
                "start = {}, end = {}, physical start = {}, physical end = {}",
                bat.start(),
                bat.end(),
                bat.physical_start(),
                bat.physical_end()
            );
            debug!(
                "start base = {:04X}, end base = {:04X}, physical start base = {:04X}, physical end base = {:04X}",
                logical_start_base, logical_end_base, physical_start_base, physical_end_base
            );

            for (i, base) in (logical_start_base..=logical_end_base).enumerate() {
                lut[base as usize] = (physical_start_base + i as u16) << 1;
            }
        }

        for bat in &mem.dbat {
            inner(&mut self.data_bat_lut, bat);
        }

        for bat in &mem.ibat {
            inner(&mut self.instr_bat_lut, bat);
        }
    }

    pub fn translate_data_addr(&self, addr: Address) -> Option<Address> {
        let addr = addr.value();
        let logical_base = addr >> 17;
        let physical_base = self.data_bat_lut[logical_base as usize] as u32;

        if physical_base == NO_BAT as u32 {
            std::hint::cold_path();
            None
        } else {
            let base = physical_base << 16;
            Some(Address(base | addr.bits(0, 17)))
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
