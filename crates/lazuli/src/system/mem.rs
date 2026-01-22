//! Memory of the system.
use std::alloc::Layout;
use std::ptr::NonNull;

use bitos::BitUtils;
use gekko::{Address, Bat, MemoryManagement};

use crate::system::ipl::Ipl;

pub const RAM_LEN: usize = 24 * bytesize::MIB as usize;
pub const L2C_LEN: usize = 16 * bytesize::KIB as usize;
pub const IPL_LEN: usize = 2 * bytesize::MIB as usize;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PageTranslation(u16);

impl PageTranslation {
    const NO_MAPPING: Self = Self(1 << 15);

    #[inline(always)]
    pub fn new(physical_base: Option<u16>) -> Self {
        physical_base.map_or(Self::NO_MAPPING, Self)
    }

    #[inline(always)]
    pub fn base(&self) -> Option<u16> {
        (*self != Self::NO_MAPPING).then_some(self.0)
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

const PAGES_COUNT: usize = 1 << 15;
type TranslationLut = [PageTranslation; PAGES_COUNT];
type FastmemLut = [Option<NonNull<u8>>; PAGES_COUNT];

enum Region {
    Ram,
    L2c,
    Ipl,
}

pub const RAM_START: u32 = 0x0000_0000;
pub const RAM_END: u32 = RAM_START + RAM_LEN as u32 - 1;
pub const L2C_START: u32 = 0xE000_0000;
pub const L2C_END: u32 = L2C_START + L2C_LEN as u32 - 1;
pub const IPL_START: u32 = 0xFFF0_0000;
pub const IPL_END: u32 = IPL_START + (IPL_LEN as u32 / 2 - 1);

impl Region {
    fn of(addr: Address) -> Option<(Self, u32)> {
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
    ram: NonNull<u8>,
    l2c: NonNull<u8>,
    ipl: NonNull<u8>,

    data_fastmem_lut_physical: Box<FastmemLut>,
    data_fastmem_lut_logical: Box<FastmemLut>,
    data_translation_lut: Box<TranslationLut>,
    inst_translation_lut: Box<TranslationLut>,
}

fn update_fastmem_lut(
    ram: *mut u8,
    l2c: *mut u8,
    ipl: *mut u8,
    lut: &mut FastmemLut,
    iter: impl IntoIterator<Item = (u32, u32)>,
) {
    for (logical_base, physical_base) in iter {
        let physical = Address(physical_base << 17);
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

        lut[logical_base as usize] = NonNull::new(ptr);
    }
}

fn update_fastmem_lut_with_bat(
    ram: *mut u8,
    l2c: *mut u8,
    ipl: *mut u8,
    lut: &mut FastmemLut,
    bat: &Bat,
) {
    let physical_start_base = bat.physical_start().value() >> 17;
    let physical_end_base = bat.physical_end().value() >> 17;
    let logical_start_base = bat.start().value() >> 17;
    let logical_end_base = bat.end().value() >> 17;

    let logical_range = logical_start_base..=logical_end_base;
    let physical_range = physical_start_base..=physical_end_base;
    let iter = logical_range.zip(physical_range);

    update_fastmem_lut(ram, l2c, ipl, lut, iter);
}

fn update_fastmem_lut_physical(ram: *mut u8, l2c: *mut u8, ipl: *mut u8, lut: &mut FastmemLut) {
    let iter = |a, b| ((a >> 17)..=(b >> 17)).map(|x| (x, x));
    let ram_iter = iter(RAM_START, RAM_END);
    let l2c_iter = iter(L2C_START, L2C_END);
    let ipl_iter = iter(IPL_START, IPL_END);
    update_fastmem_lut(ram, l2c, ipl, lut, ram_iter);
    update_fastmem_lut(ram, l2c, ipl, lut, l2c_iter);
    update_fastmem_lut(ram, l2c, ipl, lut, ipl_iter);
}

fn update_translation_lut_with(translation: &mut TranslationLut, bat: &Bat) {
    let physical_start_base = (bat.physical_start().value() >> 17) as u16;
    let physical_end_base = (bat.physical_end().value() >> 17) as u16;
    let logical_start_base = bat.start().value() >> 17;
    let logical_end_base = bat.end().value() >> 17;

    let logical_range = logical_start_base..=logical_end_base;
    let physical_range = physical_start_base..=physical_end_base;
    let iter = logical_range.zip(physical_range);

    for (logical_base, physical_base) in iter {
        translation[logical_base as usize] = PageTranslation::new(Some(physical_base));
    }
}

impl Memory {
    pub fn new(ipl_data: &Ipl) -> Self {
        let alloc = |len| {
            NonNull::new(unsafe { std::alloc::alloc(Layout::array::<u8>(len).unwrap()) }).unwrap()
        };

        let ram = alloc(RAM_LEN);
        let l2c = alloc(L2C_LEN);
        let ipl = alloc(IPL_LEN);

        unsafe {
            std::ptr::copy_nonoverlapping(ipl_data.as_ptr(), ipl.as_ptr(), IPL_LEN);
        }

        let mut data_fastmem_lut_physical = util::boxed_array(None);
        update_fastmem_lut_physical(
            ram.as_ptr(),
            l2c.as_ptr(),
            ipl.as_ptr(),
            &mut data_fastmem_lut_physical,
        );

        Self {
            ram,
            l2c,
            ipl,

            data_fastmem_lut_physical,
            data_fastmem_lut_logical: util::boxed_array(None),
            data_translation_lut: util::boxed_array(PageTranslation::NO_MAPPING),
            inst_translation_lut: util::boxed_array(PageTranslation::NO_MAPPING),
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

        self.data_fastmem_lut_logical.fill(None);
        self.data_translation_lut.fill(PageTranslation::NO_MAPPING);
        for (i, bat) in dbats.iter().enumerate() {
            if !bat.supervisor_mode() {
                tracing::warn!("dbat{i} is disabled in supervisor mode");
                continue;
            }

            update_translation_lut_with(&mut self.data_translation_lut, bat);
            update_fastmem_lut_with_bat(
                self.ram.as_ptr(),
                self.l2c.as_ptr(),
                self.ipl.as_ptr(),
                &mut self.data_fastmem_lut_logical,
                bat,
            );
        }
    }

    pub fn build_instr_bat_lut(&mut self, ibats: &[Bat; 4]) {
        let _span = tracing::info_span!("building ibat lut").entered();

        self.inst_translation_lut.fill(PageTranslation::NO_MAPPING);
        for (i, bat) in ibats.iter().enumerate() {
            if !bat.supervisor_mode() {
                tracing::warn!("ibat{i} is disabled in supervisor mode");
                continue;
            }

            update_translation_lut_with(&mut self.inst_translation_lut, bat);
        }
    }

    pub fn build_bat_lut(&mut self, memory: &MemoryManagement) {
        let _span = tracing::info_span!("building bat luts").entered();
        self.build_data_bat_lut(&memory.dbat);
        self.build_instr_bat_lut(&memory.ibat);
    }

    #[inline(always)]
    fn translate_addr(&self, lut: &TranslationLut, addr: Address) -> Option<Address> {
        let addr = addr.value();
        let logical_base = addr >> 17;
        let page = lut[logical_base as usize];
        page.translate(addr).map(Address)
    }

    pub fn translate_data_addr<A: Into<Address>>(&self, addr: A) -> Option<A>
    where
        Address: Into<A>,
    {
        self.translate_addr(&self.data_translation_lut, addr.into())
            .map(Into::into)
    }

    pub fn translate_inst_addr<A: Into<Address>>(&self, addr: A) -> Option<A>
    where
        Address: Into<A>,
    {
        self.translate_addr(&self.inst_translation_lut, addr.into())
            .map(Into::into)
    }

    /// Returns the fastmem LUT.
    #[inline(always)]
    pub fn data_fastmem_lut_logical(&self) -> &FastmemLut {
        &self.data_fastmem_lut_logical
    }

    /// Returns the fastmem LUT.
    #[inline(always)]
    pub fn data_fastmem_lut_physical(&self) -> &FastmemLut {
        &self.data_fastmem_lut_physical
    }
}

unsafe impl Send for Memory {}

impl Drop for Memory {
    fn drop(&mut self) {
        let dealloc = |ptr: NonNull<u8>, len| unsafe {
            std::alloc::dealloc(ptr.as_ptr(), Layout::array::<u8>(len).unwrap())
        };

        dealloc(self.ram, RAM_LEN);
        dealloc(self.l2c, L2C_LEN);
        dealloc(self.ipl, IPL_LEN);
    }
}
