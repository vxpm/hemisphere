//! Memory of the system.
mod alloc;

use crate::system::ipl::Ipl;
use std::{num::NonZeroUsize, ptr::NonNull};

pub const RAM_LEN: usize = 24 * bytesize::MIB as usize;
pub const L2C_LEN: usize = 16 * bytesize::KIB as usize;
pub const IPL_LEN: usize = 2 * bytesize::MIB as usize;

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
}

unsafe impl Send for Memory {}

impl Drop for Memory {
    fn drop(&mut self) {
        alloc::unmap_address_space(self.base);
    }
}
