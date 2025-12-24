//! Memory of the system.
mod alloc;

use bitos::BitUtils;
use gekko::{Address, Bat, MemoryManagement};

use crate::system::ipl::Ipl;
use std::{num::NonZeroUsize, ptr::NonNull};

pub const RAM_LEN: usize = 24 * bytesize::MIB as usize;
pub const L2C_LEN: usize = 16 * bytesize::KIB as usize;
pub const IPL_LEN: usize = 2 * bytesize::MIB as usize;

const BAT_PAGE_STRIDE: usize = 1 << 17;

#[derive(Clone, Copy)]
pub struct BatPage {
    ptr: *mut u8,
    base: Option<u16>,
}

impl BatPage {
    const NO_MAPPING: Self = Self {
        ptr: std::ptr::null_mut(),
        base: None,
    };

    #[inline(always)]
    pub fn new(ptr: *mut u8, physical_base: Option<u16>) -> Self {
        assert!(ptr.addr().is_multiple_of(BAT_PAGE_STRIDE));
        BatPage {
            ptr,
            base: physical_base,
        }
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    #[inline(always)]
    pub fn base(&self) -> Option<u16> {
        self.base
    }

    #[inline(always)]
    pub fn translate(&self, offset: u32) -> Option<u32> {
        if let Some(base) = self.base() {
            Some(offset.with_bits(17, 32, base as u32))
        } else {
            std::hint::cold_path();
            None
        }
    }
}

type PagesLut = Box<[BatPage; PAGES_COUNT]>;
const PAGES_COUNT: usize = 1 << 15;

enum Region {
    Ram,
    L2c,
    Ipl,
}

impl Region {
    fn of(addr: Address) -> Option<(Self, u32)> {
        const RAM_START: u32 = 0x0000_0000;
        const RAM_END: u32 = RAM_START + RAM_LEN as u32 - 1;
        const L2C_START: u32 = 0xE000_0000;
        const L2C_END: u32 = L2C_START + L2C_LEN as u32 - 1;
        const IPL_START: u32 = 0xFFF0_0000;
        const IPL_END: u32 = IPL_START + (IPL_LEN as u32 / 2 - 1);

        let addr = addr.value();
        Some(match addr {
            RAM_START..=RAM_END => (Self::Ram, addr - RAM_START),
            L2C_START..=L2C_END => (Self::L2c, addr - L2C_START),
            IPL_START..=IPL_END => (Self::Ipl, addr - IPL_START),
            _ => return None,
        })
    }
}

pub struct Regions<'mem> {
    pub ram: &'mem mut [u8],
    pub l2c: &'mem mut [u8],
    pub ipl: &'mem [u8],
}

pub struct Memory {
    base: NonNull<u8>,
    ram: NonNull<u8>,
    l2c: NonNull<u8>,
    ipl: NonNull<u8>,

    data_lut: PagesLut,
    inst_lut: PagesLut,
}

fn update_lut_with(ram: *mut u8, l2c: *mut u8, ipl: *mut u8, lut: &mut PagesLut, bat: &Bat) {
    let physical_start_base = (bat.physical_start().value() >> 17) as u16;
    let physical_end_base = (bat.physical_end().value() >> 17) as u16;
    let logical_start_base = bat.start().value() >> 17;
    let logical_end_base = bat.end().value() >> 17;

    let logical_range = logical_start_base..=logical_end_base;
    let physical_range = physical_start_base..=physical_end_base;
    let iter = logical_range.zip(physical_range);

    tracing::debug!(
        "start = {}, end = {}, physical start = {}, physical end = {}",
        bat.start(),
        bat.end(),
        bat.physical_start(),
        bat.physical_end()
    );
    tracing::debug!(
        "start base = {:04X}, end base = {:04X}, physical start base = {:04X}, physical end base = {:04X}",
        logical_start_base,
        logical_end_base,
        physical_start_base,
        physical_end_base
    );

    for (logical_base, physical_base) in iter {
        let physical = Address((physical_base as u32) << 17);
        let region = Region::of(physical);

        let ptr = if let Some((region, offset)) = region {
            let base = match region {
                Region::Ram => ram,
                Region::L2c => l2c,
                Region::Ipl => ipl,
            };

            unsafe { base.add(offset as usize) }
        } else {
            std::ptr::null_mut()
        };

        tracing::debug!("setting logical base {logical_base:04X} to {physical_base:04X}");
        lut[logical_base as usize] = BatPage::new(ptr, Some(physical_base));
    }
}

impl Memory {
    pub fn new(ipl_data: &Ipl) -> Self {
        let base = alloc::map_address_space();

        let ram = base;
        alloc::map_mem_at(ram, NonZeroUsize::new(RAM_LEN).unwrap());

        let l2c = unsafe { base.add(0xE000_0000) };
        alloc::map_mem_at(l2c, NonZeroUsize::new(L2C_LEN).unwrap());

        let ipl = unsafe { base.add(0xFFF0_0000) };
        alloc::map_mem_at(ipl, NonZeroUsize::new(IPL_LEN).unwrap());

        unsafe {
            std::ptr::copy_nonoverlapping(ipl_data.as_ptr(), ipl.as_ptr(), IPL_LEN);
        }

        Self {
            base,
            ram,
            l2c,
            ipl,

            data_lut: util::boxed_array(BatPage::NO_MAPPING),
            inst_lut: util::boxed_array(BatPage::NO_MAPPING),
        }
    }

    #[inline(always)]
    pub fn ram(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ram.as_ptr(), RAM_LEN) }
    }

    #[inline(always)]
    pub fn ram_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ram.as_ptr(), RAM_LEN) }
    }

    #[inline(always)]
    pub fn l2c(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.l2c.as_ptr(), L2C_LEN) }
    }

    #[inline(always)]
    pub fn l2c_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.l2c.as_ptr(), L2C_LEN) }
    }

    #[inline(always)]
    pub fn ipl(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ipl.as_ptr(), IPL_LEN) }
    }

    #[inline(always)]
    pub fn regions(&self) -> Regions<'_> {
        let ram = unsafe { std::slice::from_raw_parts_mut(self.ram.as_ptr(), RAM_LEN) };
        let l2c = unsafe { std::slice::from_raw_parts_mut(self.l2c.as_ptr(), L2C_LEN) };
        let ipl = unsafe { std::slice::from_raw_parts(self.ipl.as_ptr(), IPL_LEN) };

        Regions { ram, l2c, ipl }
    }

    pub fn build_data_bat_lut(&mut self, dbats: &[Bat; 4]) {
        let _span = tracing::info_span!("building dbat lut").entered();

        self.data_lut.fill(BatPage::NO_MAPPING);
        for (i, bat) in dbats.iter().enumerate() {
            if !bat.supervisor_mode() {
                tracing::warn!("dbat{i} is disabled in supervisor mode");
                continue;
            }

            update_lut_with(
                self.ram.as_ptr(),
                self.l2c.as_ptr(),
                self.ipl.as_ptr(),
                &mut self.data_lut,
                bat,
            );
        }
    }

    pub fn build_instr_bat_lut(&mut self, ibats: &[Bat; 4]) {
        let _span = tracing::info_span!("building ibat lut").entered();

        self.inst_lut.fill(BatPage::NO_MAPPING);
        for (i, bat) in ibats.iter().enumerate() {
            if !bat.supervisor_mode() {
                tracing::warn!("ibat{i} is disabled in supervisor mode");
                continue;
            }

            update_lut_with(
                self.ram.as_ptr(),
                self.l2c.as_ptr(),
                self.ipl.as_ptr(),
                &mut self.inst_lut,
                bat,
            );
        }
    }

    pub fn build_bat_lut(&mut self, memory: &MemoryManagement) {
        let _span = tracing::info_span!("building bat luts").entered();
        self.build_data_bat_lut(&memory.dbat);
        self.build_instr_bat_lut(&memory.ibat);
    }

    #[inline(always)]
    fn translate_addr(&self, lut: &PagesLut, addr: Address) -> Option<Address> {
        let addr = addr.value();
        let logical_base = addr >> 17;
        let page = lut[logical_base as usize];
        page.translate(addr).map(Address)
    }

    pub fn translate_data_addr<A: Into<Address>>(&self, addr: A) -> Option<A>
    where
        Address: Into<A>,
    {
        self.translate_addr(&self.data_lut, addr.into())
            .map(Into::into)
    }

    pub fn translate_inst_addr<A: Into<Address>>(&self, addr: A) -> Option<A>
    where
        Address: Into<A>,
    {
        self.translate_addr(&self.inst_lut, addr.into())
            .map(Into::into)
    }

    #[inline]
    pub fn get_data_page(&self, addr: Address) -> BatPage {
        let addr = addr.value();
        let logical_base = addr >> 17;
        self.data_lut[logical_base as usize]
    }
}

unsafe impl Send for Memory {}

impl Drop for Memory {
    fn drop(&mut self) {
        alloc::unmap_address_space(self.base);
    }
}
