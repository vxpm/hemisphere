use std::io::{Read, Seek};

/// Trait for disk modules.
///
/// Disk modules must implement [`Read`] and [`Seek`]. If a disk is not inserted, any attempt to
/// read or seek the module should return an [`std::io::Error`] with [`std::io::ErrorKind::Other`].
/// If the implementation doesn't support inserting a disk, it should return a
/// [`std::io::ErrorKind::Unsupported`] instead.
pub trait DiskModule: Read + Seek + Send {
    /// Whether a disk is inserted.
    fn has_disk(&self) -> bool;
}

/// An implementation of [`DiskModule`] which never has a disk.
#[derive(Debug, Clone, Copy)]
pub struct NopDiskModule;

impl Read for NopDiskModule {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no disk inserted",
        ))
    }
}

impl Seek for NopDiskModule {
    fn seek(&mut self, _: std::io::SeekFrom) -> std::io::Result<u64> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no disk inserted",
        ))
    }
}

impl DiskModule for NopDiskModule {
    fn has_disk(&self) -> bool {
        false
    }
}
