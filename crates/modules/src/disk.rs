use lazuli::{gcwfmt::rvz::Rvz, modules::disk::DiskModule};
use std::io::{Read, Seek, SeekFrom};

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
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
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

/// An implementation of [`DiskModule`] for .rvz disks.
#[derive(Debug)]
pub struct RvzModule<R> {
    rvz: Rvz<R>,
    position: u64,
}

impl<R> RvzModule<R> {
    pub fn new(rvz: Rvz<R>) -> Self {
        Self { rvz, position: 0 }
    }
}

impl<R> Read for RvzModule<R>
where
    R: Read + Seek,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.rvz.read(self.position, buf)?;
        self.position += buf.len() as u64;

        Ok(buf.len())
    }
}

impl<R> Seek for RvzModule<R>
where
    R: Read + Seek,
{
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        match from {
            SeekFrom::Start(x) => self.position = x,
            SeekFrom::End(x) => {
                self.position = self
                    .rvz
                    .rvz_header()
                    .inner
                    .disk_len
                    .saturating_sub_signed(x)
            }
            SeekFrom::Current(x) => self.position = self.position.saturating_add_signed(x),
        }

        Ok(self.position)
    }
}

impl<R> DiskModule for RvzModule<R>
where
    R: Read + Seek + Send,
{
    fn has_disk(&self) -> bool {
        true
    }
}
