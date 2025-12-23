use std::{num::NonZeroUsize, ptr::NonNull};

#[cfg(target_family = "unix")]
use nix::sys::mman::{self, MapFlags, ProtFlags};

const ADDR_SPACE_LENGTH: usize = 1 << 32;
const ADDR_SPACE_ALIGNMENT: usize = 1 << 17;
const HOST_PAGE_SIZE: usize = 4096;

pub fn map_address_space() -> NonNull<u8> {
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

pub fn map_mem_at(ptr: NonNull<u8>, length: NonZeroUsize) {
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

pub fn unmap_address_space(ptr: NonNull<u8>) {
    unsafe {
        mman::munmap(ptr.cast(), ADDR_SPACE_LENGTH).unwrap();
    }
}
