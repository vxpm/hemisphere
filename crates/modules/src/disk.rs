use lazuli::modules::disk::DiskModule;
use std::io::{Read, Seek};

/// An implementation of [`DiskModule`] for raw .iso data from a reader.
#[derive(Debug)]
pub struct IsoModule<R>(pub Option<R>);

impl<R> Read for IsoModule<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(r) = &mut self.0 {
            r.read(buf)
        } else {
            Err(std::io::Error::other("no disk inserted"))
        }
    }
}

impl<R> Seek for IsoModule<R>
where
    R: Seek,
{
    fn seek(&mut self, from: std::io::SeekFrom) -> std::io::Result<u64> {
        if let Some(r) = &mut self.0 {
            r.seek(from)
        } else {
            Err(std::io::Error::other("no disk inserted"))
        }
    }
}

impl<R> DiskModule for IsoModule<R>
where
    R: Read + Seek + Send,
{
    fn has_disk(&self) -> bool {
        self.0.is_some()
    }
}
