//! A `.rvz` file is a disc format designed to store the same data as `.iso` files in a
//! space-efficient manner.

use crate::{Console, iso};
use binrw::{BinRead, BinResult, binread};
use std::io::{Cursor, Read, Seek, SeekFrom};

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

#[derive(Debug, Clone, BinRead)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub beta: u8,
}

/// The header of a .rvz file.
#[derive(Debug, Clone, BinRead)]
#[br(big, magic = b"RVZ\x01")]
pub struct HeaderInner {
    pub version: Version,
    pub compatible_version: Version,
    pub disk_size: u32,
    pub disk_sha1: Sha1Hash,
    pub iso_size: u64,
    pub rvz_size: u64,
}

#[derive(Debug, Clone, BinRead)]
#[br(big)]
pub struct Header {
    pub inner: HeaderInner,
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

#[derive(Debug, Clone, BinRead)]
#[br(big)]
pub struct Disk {
    #[br(parse_with = parse_console)]
    pub console: Option<Console>,
    pub compression: Compression,
    pub compression_level: u32,
    pub chunk_length: u32,

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

#[binread(big)]
#[derive(Debug, Clone, Copy)]
pub struct DiskSection {
    #[br(temp)]
    padded_disk_offset: u64,
    #[br(temp, calc = padded_disk_offset % 0x8000)]
    disk_offset_padding: u64,

    #[br(calc = padded_disk_offset - disk_offset_padding)]
    pub disk_offset: u64,
    #[br(map = |x: u64| x + disk_offset_padding)]
    pub disk_length: u64,
    pub file_sections_index: u32,
    pub file_sections_count: u32,
}

impl DiskSection {
    pub fn contains(&self, offset: u64) -> bool {
        self.disk_offset <= offset && offset < self.disk_offset + self.disk_length
    }
}

#[derive(Clone, Copy, BinRead)]
pub struct FileSectionFormat(u32);

impl std::fmt::Debug for FileSectionFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileSectionFormat")
            .field("compressed", &self.is_compressed())
            .field("length", &self.length())
            .finish()
    }
}

impl FileSectionFormat {
    pub fn is_compressed(&self) -> bool {
        (self.0 >> 31) == 1
    }

    pub fn length(&self) -> u32 {
        self.0 & !(1 << 31)
    }
}

#[binread(big)]
#[derive(Debug, Clone, Copy)]
pub struct FileSection {
    #[br(temp)]
    file_offset_div_4: u32,

    #[br(calc = file_offset_div_4 as u64 * 4)]
    pub file_offset: u64,
    pub format: FileSectionFormat,
    pub packed_size: u32,
}

/// A .rvz file.
#[derive(Debug)]
pub struct Rvz<R> {
    header: Header,
    disk: Disk,
    disk_sections: Vec<DiskSection>,
    file_sections: Vec<FileSection>,
    reader: R,
}

fn read_disk_sections<R: Read + Seek>(
    disk: &Disk,
    mut reader: R,
) -> Result<Vec<DiskSection>, binrw::Error> {
    assert_eq!(disk.compression, Compression::Zstd);

    let mut compressed = vec![0; disk.disk_sections_size as usize];
    reader.seek(SeekFrom::Start(disk.disk_sections_offset))?;
    reader.read_exact(&mut compressed)?;

    let decompressed_size = disk.disk_sections_count as usize * size_of::<DiskSection>();
    let decompressed = zstd::bulk::decompress(&compressed, decompressed_size).unwrap();

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

fn read_file_sections<R: Read + Seek>(
    disk: &Disk,
    mut reader: R,
) -> Result<Vec<FileSection>, binrw::Error> {
    assert_eq!(disk.compression, Compression::Zstd);

    let mut compressed = vec![0; disk.file_sections_size as usize];
    reader.seek(SeekFrom::Start(disk.file_sections_offset))?;
    reader.read_exact(&mut compressed)?;

    let decompressed_size = disk.file_sections_count as usize * size_of::<FileSection>();
    let decompressed = zstd::bulk::decompress(&compressed, decompressed_size).unwrap();

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
    section: FileSection,
    computed_start: u64,
    computed_length: u64,
}

#[derive(Clone, Copy, BinRead)]
pub struct PackedFormat(u32);

impl PackedFormat {
    pub fn is_padding(&self) -> bool {
        (self.0 >> 31) == 1
    }

    pub fn length(&self) -> u32 {
        self.0 & !(1 << 31)
    }
}

fn prgn_advance(buffer: &mut [u32; 521]) {
    for i in 0..32 {
        buffer[i] ^= buffer[i + 521 - 32];
    }

    for i in 32..521 {
        buffer[i] ^= buffer[i - 32];
    }
}

fn prng(seed: &[u32; 17], count: u32, offset: u32, output: &mut Vec<u8>) {
    let mut buffer = [0; 521];
    buffer[..17].copy_from_slice(seed);

    for i in 17..512 {
        buffer[i] = (buffer[i - 17] << 23) ^ (buffer[i - 16] >> 9) ^ buffer[i - 1];
    }

    for _ in 0..4 {
        prgn_advance(&mut buffer);
    }

    let discard = offset % 0x8000;
    let end = count + discard;

    let mut current = 0;
    while current != end {
        if current >= discard {
            let index = ((current / 4) % 521) as usize;
            let offset = current % 4;

            let value = match offset {
                0 => (buffer[index] >> 24) as u8,
                1 => (buffer[index] >> 18) as u8,
                2 => (buffer[index] >> 8) as u8,
                3 => buffer[index] as u8,
                _ => unreachable!(),
            };

            output.push(value);
        }

        current += 1;
        if current % (4 * 521) == 0 {
            prgn_advance(&mut buffer);
        }
    }
}

fn unpack(data: &[u8], offset: u32) -> Vec<u8> {
    let mut cursor = Cursor::new(data);
    let mut output = Vec::with_capacity(data.len());

    while cursor.position() != data.len() as u64 {
        let format = PackedFormat::read_be(&mut cursor).unwrap();
        if format.is_padding() {
            let seed = <[u32; 17]>::read_be(&mut cursor).unwrap();
            prng(&seed, format.length(), offset, &mut output)
        } else {
            let start = output.len();
            let len = format.length() as usize;
            output.resize(start + len, 0);
            cursor.read_exact(&mut output[start..][..len]).unwrap();
        }
    }

    output
}

impl<R> Rvz<R>
where
    R: Read + Seek,
{
    pub fn new(mut reader: R) -> Result<Self, binrw::Error> {
        let header = Header::read(&mut reader)?;
        let disk = Disk::read(&mut reader)?;
        let disk_sections = read_disk_sections(&disk, &mut reader)?;
        let file_sections = read_file_sections(&disk, &mut reader)?;

        Ok(Self {
            header,
            disk,
            disk_sections,
            file_sections,
            reader,
        })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn disk(&self) -> &Disk {
        &self.disk
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

    pub fn find_disk_section(&self, offset: u64) -> Option<DiskSection> {
        self.disk_sections
            .iter()
            .find(|x| x.contains(offset))
            .copied()
    }

    fn find_file_section(
        &self,
        index: u32,
        count: u32,
        total_length: u64,
        offset: u64,
    ) -> Option<FoundFileSection> {
        let sections = &self.file_sections[index as usize..][..count as usize];

        let mut start = 0;
        let mut iter = sections.iter().peekable();
        while let Some(section) = iter.next() {
            let length = if iter.peek().is_some() {
                self.disk.chunk_length as u64
            } else {
                total_length % self.disk.chunk_length as u64
            };

            let length = if length == 0 {
                self.disk.chunk_length as u64
            } else {
                length
            };

            if start <= offset && offset < start + length {
                return Some(FoundFileSection {
                    section: *section,
                    computed_start: start,
                    computed_length: length,
                });
            }

            start += length;
        }

        None
    }

    pub fn read_disk_section(
        &mut self,
        disk_section: DiskSection,
        disk_section_offset: u64,
        out: &mut [u8],
    ) -> Result<(), std::io::Error> {
        let mut current_disk_section_offset = disk_section_offset;
        let mut remaining = out.len() as u64;

        while remaining > 0 {
            // find file section containing current offset
            let Some(found) = self.find_file_section(
                disk_section.file_sections_index,
                disk_section.file_sections_count,
                disk_section.disk_length,
                current_disk_section_offset,
            ) else {
                dbg!(disk_section_offset, out.len());
                panic!(
                    "file section containing offset {} not found :(",
                    current_disk_section_offset
                );
            };

            // 01. read compressed data
            let length = found.section.format.length() as usize;
            let mut compressed = vec![0; length];

            if length != 0 {
                self.reader
                    .seek(SeekFrom::Start(found.section.file_offset))?;
                self.reader.read_exact(&mut compressed)?;
            }

            // 02. decompress
            let decompressed = if found.section.format.is_compressed() {
                zstd::bulk::decompress(&compressed, found.computed_length as usize).unwrap()
            } else {
                compressed
            };

            // 03. unpack
            let unpacked = if found.section.packed_size != 0 {
                assert_eq!(decompressed.len() as u64, found.section.packed_size as u64);
                unpack(&decompressed, disk_section.disk_offset as u32)
            } else {
                decompressed
            };

            // 04. copy to output
            let file_section_offset = current_disk_section_offset - found.computed_start;
            let to_read = remaining.min(found.computed_length - file_section_offset);

            let out_start = current_disk_section_offset - disk_section_offset;
            let out = &mut out[out_start as usize..][..to_read as usize];
            out.copy_from_slice(&unpacked[file_section_offset as usize..][..to_read as usize]);

            current_disk_section_offset += to_read;
            remaining -= to_read;
        }

        Ok(())
    }

    pub fn read(&mut self, disk_offset: u64, out: &mut [u8]) -> Result<(), std::io::Error> {
        let mut current = disk_offset;
        let mut remaining = out.len() as u64;

        while remaining > 0 {
            let Some(section) = self.find_disk_section(current) else {
                panic!("disk section not found :(");
            };

            // read as many bytes as possible from the section
            let section_offset = current - section.disk_offset;
            let remaining_section_length = section.disk_length - section_offset;
            let to_read = remaining.min(remaining_section_length);

            let out_start = current - disk_offset;
            let out = &mut out[out_start as usize..][..to_read as usize];
            self.read_disk_section(section, section_offset, out)?;

            // advance
            current += to_read;
            remaining -= to_read;
        }

        Ok(())
    }
}
