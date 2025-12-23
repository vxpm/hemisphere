//! Memory of the system.
use crate::system::ipl::Ipl;
use std::{num::NonZeroUsize, ptr::NonNull};

#[cfg(target_family = "unix")]
use nix::sys::mman::{self, MapFlags, ProtFlags};

pub const RAM_LEN: usize = 24 * bytesize::MIB as usize;
pub const L2C_LEN: usize = 16 * bytesize::KIB as usize;
pub const IPL_LEN: usize = 2 * bytesize::MIB as usize;

const ADDR_SPACE_LENGTH: usize = 1 << 32;
const ADDR_SPACE_ALIGNMENT: usize = 1 << 17;
const HOST_PAGE_SIZE: usize = 4096;

fn map_address_space() -> NonNull<u8> {
    // map (size + alignemnt) bytes
    let mapped = unsafe {
        mman::mmap_anonymous(
            None,
            NonZeroUsize::new(ADDR_SPACE_LENGTH + ADDR_SPACE_ALIGNMENT).unwrap(),
            ProtFlags::PROT_NONE,
            MapFlags::MAP_PRIVATE,
        )
        .unwrap()
    };

    // find aligned address
    let aligned = mapped
        .map_addr(|a| NonZeroUsize::new(a.get().next_multiple_of(ADDR_SPACE_ALIGNMENT)).unwrap());

    // unmap pages before
    let delta = unsafe { aligned.offset_from_unsigned(mapped) };
    if delta != 0 {
        unsafe {
            mman::munmap(mapped, delta - 1).unwrap();
        }
    }

    assert!(aligned.addr().get().is_multiple_of(ADDR_SPACE_ALIGNMENT));

    aligned.cast()
}

fn map_mem_at(ptr: NonNull<u8>, length: NonZeroUsize) {
    assert!(ptr.addr().get().is_multiple_of(HOST_PAGE_SIZE));

    let allocated = unsafe {
        mman::mmap_anonymous(
            Some(ptr.addr()),
            length,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_PRIVATE | MapFlags::MAP_FIXED,
        )
        .unwrap()
    };

    assert_eq!(allocated.cast(), ptr);
}

fn unmap_address_space(ptr: NonNull<u8>) {
    unsafe {
        mman::munmap(ptr.cast(), ADDR_SPACE_LENGTH).unwrap();
    }
}

pub struct Regions<'mem> {
    pub ram: &'mem mut [u8],
    pub l2c: &'mem mut [u8],
    pub ipl: &'mem [u8],
}

pub struct Memory {
    space: NonNull<u8>,
    ram: NonNull<u8>,
    l2c: NonNull<u8>,
    ipl: NonNull<u8>,
}

impl Memory {
    pub fn new(ipl_data: &Ipl) -> Self {
        let space = map_address_space();

        let ram = space;
        map_mem_at(ram, NonZeroUsize::new(RAM_LEN).unwrap());

        let l2c = unsafe { space.add(0xE000_0000) };
        map_mem_at(l2c, NonZeroUsize::new(L2C_LEN).unwrap());

        let ipl = unsafe { space.add(0xFFF0_0000) };
        map_mem_at(ipl, NonZeroUsize::new(IPL_LEN).unwrap());

        unsafe {
            std::ptr::copy_nonoverlapping(ipl_data.as_ptr(), ipl.as_ptr(), IPL_LEN);
        }

        Self {
            space,
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
        unmap_address_space(self.space);
    }
}
