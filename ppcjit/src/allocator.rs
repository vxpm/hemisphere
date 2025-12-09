use nix::sys::mman::{self, MapFlags, ProtFlags};
use std::{
    marker::PhantomData,
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

const REGION_MIN_LEN: usize = 1 << 16;

/// A memory mapped region.
#[derive(Clone, Copy)]
struct Region {
    ptr: NonNull<u8>,
    len: usize,
}

impl Region {
    pub fn new(addr_hint: Option<NonZeroUsize>, len: usize) -> Self {
        let len = len.max(REGION_MIN_LEN);
        let region = unsafe {
            mman::mmap_anonymous(
                addr_hint,
                NonZeroUsize::new(len).unwrap(),
                ProtFlags::PROT_NONE,
                MapFlags::MAP_PRIVATE,
            )
        }
        .unwrap();

        Self {
            ptr: region.cast(),
            len,
        }
    }
}

pub struct Allocation<K>(NonNull<[u8]>, PhantomData<K>);

impl<K> Allocation<K> {
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { self.0.as_ref() }
    }
}

impl Allocation<ReadWrite> {
    #[inline(always)]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { self.0.as_mut() }
    }
}

impl<K> Deref for Allocation<K> {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl DerefMut for Allocation<ReadWrite> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_bytes_mut()
    }
}

pub trait AllocKind {
    fn flags() -> ProtFlags;
}

pub struct Exec;
impl AllocKind for Exec {
    fn flags() -> ProtFlags {
        ProtFlags::PROT_READ | ProtFlags::PROT_EXEC
    }
}

pub struct ReadWrite;
impl AllocKind for ReadWrite {
    fn flags() -> ProtFlags {
        ProtFlags::PROT_READ | ProtFlags::PROT_WRITE
    }
}

pub struct Allocator<K> {
    /// The currently active region
    current: Option<Region>,
    /// Offset into the current region
    offset: usize,
    /// Phantom
    _phantom: PhantomData<K>,
}

impl<K> Allocator<K>
where
    K: AllocKind,
{
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            current: None,
            offset: 0,
            _phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn current(&mut self, len: usize) -> Region {
        if let Some(region) = self.current {
            region
        } else {
            let region = Region::new(None, len);
            self.current = Some(region);
            region
        }
    }

    fn allocate_inner(&mut self, alignment: usize, length: usize) -> (Region, Allocation<K>) {
        assert!(length > 0);

        let alignment = alignment.max(1).next_power_of_two();
        let effective_offset = self.offset.next_multiple_of(alignment);

        let region = self.current(length);
        let remaining = region.len.checked_sub(effective_offset);

        if remaining.is_none_or(|r| r < length) {
            let end = unsafe { region.ptr.add(region.len) };
            let region = Region::new(Some(end.addr()), length);
            self.current = Some(region);
            self.offset = 0;
            return self.allocate_inner(alignment, length);
        }

        let start = unsafe { region.ptr.add(effective_offset) };
        self.offset = effective_offset + length;

        (
            region,
            Allocation(NonNull::slice_from_raw_parts(start, length), PhantomData),
        )
    }

    pub fn allocate_uninit(&mut self, alignment: usize, length: usize) -> Allocation<K> {
        let (region, alloc) = self.allocate_inner(alignment, length);

        unsafe {
            mman::mprotect(region.ptr.cast(), self.offset, K::flags()).unwrap();
        }

        alloc
    }

    pub fn allocate(&mut self, alignment: usize, data: &[u8]) -> Allocation<K> {
        let (region, alloc) = self.allocate_inner(alignment, data.len());

        unsafe {
            mman::mprotect(region.ptr.cast(), self.offset, ProtFlags::PROT_WRITE).unwrap();
            std::ptr::copy_nonoverlapping(data.as_ptr(), alloc.0.as_ptr().cast(), data.len());
            mman::mprotect(region.ptr.cast(), self.offset, K::flags()).unwrap();
        }

        alloc
    }
}
