//! A `.rvz` file is a disc format designed to store the same data as `.iso` files in a
//! space-efficient manner.

use crate::{Console, apploader, dol, iso};
use binrw::{BinRead, BinResult, binread};
use easyerr::{Error, ResultExt};
use std::io::{Cursor, Read, Seek, SeekFrom};

/// A SHA1 hash.
#[derive(Clone, BinRead)]
pub struct Sha1Hash(pub [u8; 20]);

impl std::fmt::Debug for Sha1Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02X}")?;
        }

        Ok(())
    }
}

/// Version of a RVZ file.
#[derive(Debug, Clone, BinRead)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub beta: u8,
}

/// The actual header of a RVZ file.
#[derive(Debug, Clone, BinRead)]
#[br(big, magic = b"RVZ\x01")]
pub struct RvzHeaderInner {
    /// Version of this RVZ.
    pub version: Version,
    /// Version that supports reading this RVZ.
    pub compatible_version: Version,
    /// The length of the disk header.
    pub disk_header_len: u32,
    /// The SHA1 hash of the disk header.
    pub disk_header_sha1: Sha1Hash,
    /// The length of the disk this RVZ contains.
    pub disk_len: u64,
    /// The length of this RVZ.
    pub rvz_len: u64,
}

/// The header of a .rvz file. This is a wrapper around [`RvzHeaderInner`] which also contains it's
/// hash.
#[derive(Debug, Clone, BinRead)]
#[br(big)]
pub struct RvzHeader {
    /// The actual contents of the header.
    pub inner: RvzHeaderInner,
    /// The SHA1 hash of the inner field.
    pub hash: Sha1Hash,
}

#[binrw::parser(reader, endian)]
fn parse_console() -> BinResult<Option<Console>> {
    let console = u32::read_options(reader, endian, ())?;

    Ok(match console {
        1 => Some(Console::GameCube),
        2 => Some(Console::Wii),
        _ => None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, BinRead)]
#[br(repr = u32)]
pub enum Compression {
    None,
    Purge,
    Bzip2,
    Lzma,
    Lzma2,
    Zstd,
}

/// Header describing the structure of disk data in a RVZ file.
#[derive(Debug, Clone, BinRead)]
#[br(big)]
pub struct DiskHeader {
    #[br(parse_with = parse_console)]
    pub console: Option<Console>,
    pub compression: Compression,
    pub compression_level: u32,
    pub chunk_len: u32,

    #[brw(pad_size_to = 0x80)]
    #[brw(assert(disk_meta.game_name.len() <= 0x60))]
    pub disk_meta: iso::Meta,

    pub partitions_count: u32,
    pub partitions_size: u32,
    pub partitions_offset: u64,
    pub partitions_sha1: Sha1Hash,

    pub disk_sections_count: u32,
    pub disk_sections_offset: u64,
    pub disk_sections_size: u32,

    pub file_sections_count: u32,
    pub file_sections_offset: u64,
    pub file_sections_size: u32,

    pub compressor_data_count: u8,
    pub compressor_data: [u8; 7],
}

/// A disk section describes a specific range of raw (i.e. not partitioned) disk data by mapping it
/// into a sequence of file sections.
#[binread(big)]
#[derive(Debug, Clone, Copy)]
pub struct DiskSection {
    #[br(temp)]
    padded_disk_offset: u64,
    #[br(temp, calc = padded_disk_offset % 0x8000)]
    disk_offset_padding: u64,

    /// The disk offset this section refers to.
    #[br(calc = padded_disk_offset - disk_offset_padding)]
    pub disk_offset: u64,
    /// The length of the disk section.
    #[br(map = |x: u64| x + disk_offset_padding)]
    pub disk_len: u64,
    /// The index of the starting file section sequence that contains the data of this disk section.
    pub file_sections_index: u32,
    /// The length of the file sections sequence that contains the data of this disk section.
    pub file_sections_count: u32,
}

impl DiskSection {
    /// Whether this disk section contains the given offset into the disk.
    pub fn contains(&self, disk_offset: u64) -> bool {
        self.disk_offset <= disk_offset && disk_offset < self.disk_offset + self.disk_len
    }
}

/// The format of the compression of a file section.
#[derive(Clone, Copy, BinRead)]
pub struct CompressionFormat(u32);

impl std::fmt::Debug for CompressionFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompressionFormat")
            .field("compressed", &self.is_compressed())
            .field("len", &self.len())
            .finish()
    }
}

impl CompressionFormat {
    /// Length of this file section in the RVZ.
    pub fn len(&self) -> u32 {
        self.0 & !(1 << 31)
    }

    /// Whether this file section is compressed.
    pub fn is_compressed(&self) -> bool {
        (self.0 >> 31) == 1
    }

    /// Whether this file section is zeroed (i.e. all of it's bytes are zero).
    pub fn is_zeroed(&self) -> bool {
        self.len() == 0
    }
}

/// The format of packed data in a file section.
#[derive(Clone, Copy, BinRead)]
pub struct PackingFormat(u32);

impl std::fmt::Debug for PackingFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackingFormat")
            .field("packed", &self.is_packed())
            .field("len", &self.len())
            .finish()
    }
}

impl PackingFormat {
    /// Whether the data in the file section is packed.
    pub fn is_packed(&self) -> bool {
        self.0 != 0
    }

    /// The length of the packed data.
    pub fn len(&self) -> u32 {
        self.0
    }
}

/// A file section describes a specific range of data in the RVZ file.
#[binread(big)]
#[derive(Debug, Clone, Copy)]
pub struct FileSection {
    #[br(temp)]
    file_offset_div_4: u32,

    /// The file offset this section refers to.
    #[br(calc = file_offset_div_4 as u64 * 4)]
    pub file_offset: u64,
    /// The format of the compressed data of this file section.
    pub compression: CompressionFormat,
    /// The format of the packed data of this file section.
    pub packing: PackingFormat,
}

/// A descriptor for a chunk of packed data.
#[derive(Clone, Copy, BinRead)]
pub struct PackedChunk(u32);

impl PackedChunk {
    /// Whether the data is padding.
    pub fn is_padding(&self) -> bool {
        (self.0 >> 31) == 1
    }

    /// The length of the chunk.
    pub fn len(&self) -> u32 {
        self.0 & !(1 << 31)
    }
}

/// Implementation of the PRNG used for generating padding data: a lagged fibonacci generator with
/// parameters f = xor, j = 32 and k = 521.
struct Prng {
    buffer: [u32; 521],
    current: usize,
}

impl Prng {
    const SEED_LEN: usize = 17;
    const BUF_LEN: usize = 521;

    /// Creates a PRNG instance from the given seed data.
    fn from_seed(seed: &[u32; Self::SEED_LEN]) -> Self {
        let mut buffer = [0; Self::BUF_LEN];
        buffer[..Self::SEED_LEN].copy_from_slice(seed);

        for i in Self::SEED_LEN..Self::BUF_LEN {
            buffer[i] = (buffer[i - 17] << 23) ^ (buffer[i - 16] >> 9) ^ buffer[i - 1];
        }

        let mut prng = Self { buffer, current: 0 };

        prng.advance();
        prng.advance();
        prng.advance();
        prng.advance();

        prng
    }

    /// Advances the internal PRNG buffer, generating the next 2084 bytes of data.
    fn advance(&mut self) {
        for i in 0..32 {
            self.buffer[i] ^= self.buffer[i + Self::BUF_LEN - 32];
        }

        for i in 32..Self::BUF_LEN {
            self.buffer[i] ^= self.buffer[i - 32];
        }

        self.current = 0;
    }

    /// Get the next byte of PRNG data. If necessary, advances the internal buffer.
    fn next(&mut self) -> u8 {
        if self.current == 4 * Self::BUF_LEN {
            self.advance();
        }

        let index = (self.current / 4) % Self::BUF_LEN;
        let offset = self.current % 4;

        let value = match offset {
            0 => (self.buffer[index] >> 24) as u8,
            1 => (self.buffer[index] >> 18) as u8,
            2 => (self.buffer[index] >> 8) as u8,
            3 => self.buffer[index] as u8,
            _ => unreachable!(),
        };

        self.current += 1;

        value
    }

    fn skip(&mut self, count: usize) {
        for _ in 0..count / Self::SEED_LEN {
            self.advance();
        }

        let remainder = count % Self::SEED_LEN;
        self.current += remainder;
        if self.current >= Self::SEED_LEN {
            self.advance();
            self.current %= Self::SEED_LEN;
        }
    }
}

/// Unpacks a sequence of bytes at the given offset.
fn unpack(data: &[u8], offset: u32) -> Vec<u8> {
    let mut cursor = Cursor::new(data);
    let mut output = Vec::with_capacity(data.len());

    while cursor.position() != data.len() as u64 {
        let format = PackedChunk::read_be(&mut cursor).unwrap();
        if format.is_padding() {
            let seed = <[u32; 17]>::read_be(&mut cursor).unwrap();
            let discard = offset % 0x8000;

            let mut prng = Prng::from_seed(&seed);
            prng.skip(discard as usize);

            for _ in 0..format.len() {
                output.push(prng.next());
            }
        } else {
            let start = output.len();
            let len = format.len() as usize;
            output.resize(start + len, 0);
            cursor.read_exact(&mut output[start..][..len]).unwrap();
        }
    }

    output
}

enum Decompressor {
    None,
    Zstd(zstd::bulk::Decompressor<'static>),
}

impl Decompressor {
    fn decompress(&mut self, data: &[u8], length: usize) -> Vec<u8> {
        match self {
            Self::None => data.to_vec(),
            Self::Zstd(decompressor) => decompressor.decompress(data, length).unwrap(),
        }
    }
}

/// Reads the disk sections in a RVZ.
fn read_disk_sections<R: Read + Seek>(
    disk: &DiskHeader,
    decompressor: &mut Decompressor,
    mut reader: R,
) -> Result<Vec<DiskSection>, binrw::Error> {
    assert_eq!(disk.compression, Compression::Zstd);

    let mut compressed = vec![0; disk.disk_sections_size as usize];
    reader.seek(SeekFrom::Start(disk.disk_sections_offset))?;
    reader.read_exact(&mut compressed)?;

    let decompressed_size = disk.disk_sections_count as usize * size_of::<DiskSection>();
    let decompressed = decompressor.decompress(&compressed, decompressed_size);

    let mut cursor = Cursor::new(decompressed);
    let decoded = <Vec<DiskSection>>::read_options(
        &mut cursor,
        binrw::endian::BE,
        binrw::VecArgs::builder()
            .count(disk.disk_sections_count as usize)
            .finalize(),
    )?;

    Ok(decoded)
}

/// Reads the file sections in a RVZ.
fn read_file_sections<R: Read + Seek>(
    disk: &DiskHeader,
    decompressor: &mut Decompressor,
    mut reader: R,
) -> Result<Vec<FileSection>, binrw::Error> {
    let mut compressed = vec![0; disk.file_sections_size as usize];
    reader.seek(SeekFrom::Start(disk.file_sections_offset))?;
    reader.read_exact(&mut compressed)?;

    let decompressed_size = disk.file_sections_count as usize * size_of::<FileSection>();
    let decompressed = decompressor.decompress(&compressed, decompressed_size);

    let mut cursor = Cursor::new(decompressed);
    let decoded = <Vec<FileSection>>::read_options(
        &mut cursor,
        binrw::endian::BE,
        binrw::VecArgs::builder()
            .count(disk.file_sections_count as usize)
            .finalize(),
    )?;

    Ok(decoded)
}

struct FoundFileSection {
    inner: FileSection,
    disk_start: u64,
    disk_len: u64,
}

#[derive(Debug, Error)]
pub enum RvzError {
    #[error("unsupported compression format {f0:?}")]
    UnsupportedCompression(Compression),
    #[error(transparent)]
    ParsingRvzHeader { source: binrw::Error },
    #[error(transparent)]
    ParsingDiskHeader { source: binrw::Error },
    #[error(transparent)]
    ParsingDiskSections { source: binrw::Error },
    #[error(transparent)]
    ParsingFileSections { source: binrw::Error },
    #[error(transparent)]
    ReadingFileSection { source: std::io::Error },
    #[error(
        "file section containing offset {disk_section_offset} of {disk_section:?} could not be found"
    )]
    FileSectionNotFound {
        disk_section: DiskSection,
        disk_section_offset: u64,
    },
}

/// A .rvz file.
pub struct Rvz<R> {
    rvz_header: RvzHeader,
    disk_header: DiskHeader,
    disk_sections: Vec<DiskSection>,
    file_sections: Vec<FileSection>,
    decompressor: Decompressor,
    reader: R,
}

impl<R> Rvz<R>
where
    R: Read + Seek,
{
    /// Creates a new [`Rvz`] from the given reader. This function _does not_ validate the RVZ,
    /// i.e. hashes are not computed and checked.
    pub fn new(mut reader: R) -> Result<Self, RvzError> {
        let header = RvzHeader::read(&mut reader).context(RvzCtx::ParsingRvzHeader)?;
        let disk = DiskHeader::read(&mut reader).context(RvzCtx::ParsingDiskHeader)?;

        let mut decompressor = match disk.compression {
            Compression::None => Decompressor::None,
            Compression::Zstd => Decompressor::Zstd(zstd::bulk::Decompressor::new().unwrap()),
            _ => return Err(RvzError::UnsupportedCompression(disk.compression)),
        };

        let disk_sections = read_disk_sections(&disk, &mut decompressor, &mut reader)
            .context(RvzCtx::ParsingDiskSections)?;
        let file_sections = read_file_sections(&disk, &mut decompressor, &mut reader)
            .context(RvzCtx::ParsingFileSections)?;

        Ok(Self {
            rvz_header: header,
            disk_header: disk,
            disk_sections,
            file_sections,
            decompressor,
            reader,
        })
    }

    pub fn rvz_header(&self) -> &RvzHeader {
        &self.rvz_header
    }

    pub fn disk_header(&self) -> &DiskHeader {
        &self.disk_header
    }

    pub fn disk_sections(&self) -> &[DiskSection] {
        &self.disk_sections
    }

    pub fn file_sections(&self) -> &[FileSection] {
        &self.file_sections
    }

    pub fn reader(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Finds the disk section that contains the given disk offset.
    pub fn find_disk_section(&self, disk_offset: u64) -> Option<DiskSection> {
        self.disk_sections
            .iter()
            .find(|x| x.contains(disk_offset))
            .copied()
    }

    /// Finds the file section that contains the given offset into it's disk section.
    fn find_file_section(
        &self,
        disk_section: DiskSection,
        disk_section_offset: u64,
    ) -> Option<FoundFileSection> {
        let chunk_len = self.disk_header.chunk_len as u64;
        let file_section_idx = disk_section_offset / chunk_len;
        let file_section_disk_start = file_section_idx * chunk_len;
        let file_section_disk_len =
            (disk_section.disk_len - file_section_disk_start).min(chunk_len);

        if file_section_idx < disk_section.file_sections_count as u64 {
            let file_section_idx = disk_section.file_sections_index as u64 + file_section_idx;
            Some(FoundFileSection {
                inner: self.file_sections[file_section_idx as usize],
                disk_start: file_section_disk_start,
                disk_len: file_section_disk_len,
            })
        } else {
            None
        }
    }

    /// Reads a disk section at the given offset and writes it into the output buffer.
    pub fn read_disk_section(
        &mut self,
        disk_section: DiskSection,
        disk_section_offset: u64,
        out: &mut [u8],
    ) -> Result<(), RvzError> {
        let mut current_disk_section_offset = disk_section_offset;
        let mut remaining = out.len() as u64;

        while remaining > 0 {
            // find file section containing current offset
            let Some(section) = self.find_file_section(disk_section, current_disk_section_offset)
            else {
                return Err(RvzError::FileSectionNotFound {
                    disk_section,
                    disk_section_offset: current_disk_section_offset,
                });
            };

            // 01. read compressed data
            let compression = section.inner.compression;
            let compressed = if compression.is_zeroed() {
                let len = section.disk_len as usize;
                vec![0; len]
            } else {
                let len = compression.len() as usize;
                let mut compressed = vec![0; len];

                self.reader
                    .seek(SeekFrom::Start(section.inner.file_offset))
                    .context(RvzCtx::ReadingFileSection)?;

                self.reader
                    .read_exact(&mut compressed)
                    .context(RvzCtx::ReadingFileSection)?;

                compressed
            };

            // 02. decompress
            let decompressed = if !compression.is_zeroed() && compression.is_compressed() {
                self.decompressor
                    .decompress(&compressed, section.disk_len as usize)
            } else {
                compressed
            };

            // 03. unpack
            let packing = section.inner.packing;
            let unpacked = if packing.is_packed() {
                assert_eq!(decompressed.len() as u64, packing.len() as u64);
                unpack(&decompressed, disk_section.disk_offset as u32)
            } else {
                decompressed
            };

            // 04. copy to output
            let file_section_offset = current_disk_section_offset - section.disk_start;
            let to_read = remaining.min(section.disk_len - file_section_offset);

            let out_start = current_disk_section_offset - disk_section_offset;
            let out = &mut out[out_start as usize..][..to_read as usize];
            out.copy_from_slice(&unpacked[file_section_offset as usize..][..to_read as usize]);

            current_disk_section_offset += to_read;
            remaining -= to_read;
        }

        Ok(())
    }

    /// Reads from disk at the given offset and writes it into the output buffer. Returns how many
    /// bytes were actually read.
    pub fn read(&mut self, disk_offset: u64, out: &mut [u8]) -> Result<u64, RvzError> {
        let mut current_disk_offset = disk_offset;
        let mut remaining = out.len() as u64;

        while remaining > 0 {
            let Some(section) = self.find_disk_section(current_disk_offset) else {
                break;
            };

            // read as many bytes as possible from the section
            let section_offset = current_disk_offset - section.disk_offset;
            let remaining_section_len = section.disk_len - section_offset;
            let to_read = remaining.min(remaining_section_len);

            let out_start = current_disk_offset - disk_offset;
            let out = &mut out[out_start as usize..][..to_read as usize];
            self.read_disk_section(section, section_offset, out)?;

            // advance
            current_disk_offset += to_read;
            remaining -= to_read;
        }

        Ok(out.len() as u64 - remaining)
    }
}

/// A wrapper around [`Rvz`] providing an implementation of [`Read`] and [`Seek`].
pub struct RvzReader<R> {
    rvz: Rvz<R>,
    position: u64,
}

impl<R> RvzReader<R> {
    pub fn new(rvz: Rvz<R>) -> Self {
        Self { rvz, position: 0 }
    }

    pub fn inner(&self) -> &Rvz<R> {
        &self.rvz
    }

    pub fn inner_mut(&mut self) -> &mut Rvz<R> {
        &mut self.rvz
    }

    pub fn into_inner(self) -> Rvz<R> {
        self.rvz
    }
}

impl<R> Read for RvzReader<R>
where
    R: Read + Seek,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = match self.rvz.read(self.position, buf) {
            Ok(read) => read,
            Err(e) => {
                return Err(std::io::Error::other(format!(
                    "rvz disk module failed: {e}"
                )));
            }
        };

        self.position += read;
        Ok(read as usize)
    }
}

impl<R> Seek for RvzReader<R>
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

impl<R> RvzReader<R>
where
    R: Read + Seek,
{
    pub fn iso_header(&mut self) -> Result<iso::Header, binrw::Error> {
        self.seek(SeekFrom::Start(0))?;
        iso::Header::read_be(self)
    }

    pub fn bootfile(&mut self) -> Result<dol::Dol, binrw::Error> {
        let header = self.iso_header()?;
        self.seek(SeekFrom::Start(header.bootfile_offset as u64))?;
        dol::Dol::read(self)
    }

    pub fn bootfile_header(&mut self) -> Result<dol::Header, binrw::Error> {
        let header = self.iso_header()?;
        self.seek(SeekFrom::Start(header.bootfile_offset as u64))?;
        dol::Header::read(self)
    }

    pub fn apploader(&mut self) -> Result<apploader::Apploader, binrw::Error> {
        self.seek(SeekFrom::Start(0x2440))?;
        apploader::Apploader::read(self)
    }

    pub fn apploader_header(&mut self) -> Result<apploader::Header, binrw::Error> {
        self.seek(SeekFrom::Start(0x2440))?;
        apploader::Header::read(self)
    }

    pub fn filesystem(&mut self) -> Result<iso::filesystem::FileSystem, binrw::Error> {
        let header = self.iso_header()?;
        self.seek(SeekFrom::Start(header.filesystem_offset as u64))?;
        iso::filesystem::FileSystem::read(self)
    }
}
