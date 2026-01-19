//! A `.dol` file is a proprietary executable format used in the GameCube and the Wii.

use std::io::{Read, Seek};

use binrw::{BinRead, BinWrite};
use easyerr::{Error, ResultExt};

const HEADER_SIZE: usize = 0x100;

// The header of a .dol file.
#[derive(Debug, Default, BinRead, BinWrite)]
#[brw(big)]
pub struct Header {
    /// Offset of the start of .text sections. An offset of zero means the section does not exist.
    pub text_offsets: [u32; 7],
    /// Offset of the start of .data sections. An offset of zero means the section does not exist.
    pub data_offsets: [u32; 11],
    /// Address where the .text sections should be loaded at.
    pub text_targets: [u32; 7],
    /// Address where the .data sections should be loaded at.
    pub data_targets: [u32; 11],
    /// Size of each .text section.
    pub text_sizes: [u32; 7],
    /// Size of each .data section.
    pub data_sizes: [u32; 11],
    /// Offset of the bss section.
    pub bss_target: u32,
    /// Size of the bss section.
    pub bss_size: u32,
    /// Entrypoint of the executable.
    pub entry: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct SectionInfo {
    pub offset: u32,
    pub target: u32,
    pub size: u32,
}

impl Header {
    pub fn text_sections(&self) -> impl Iterator<Item = SectionInfo> {
        (0..7).filter_map(|i| {
            let offset = self.text_offsets[i];
            if offset == 0 {
                return None;
            }

            let target = self.text_targets[i];
            let size = self.text_sizes[i];

            Some(SectionInfo {
                offset,
                target,
                size,
            })
        })
    }

    pub fn data_sections(&self) -> impl Iterator<Item = SectionInfo> {
        (0..11).filter_map(|i| {
            let offset = self.data_offsets[i];
            if offset == 0 {
                return None;
            }

            let target = self.data_targets[i];
            let size = self.data_sizes[i];

            Some(SectionInfo {
                offset,
                target,
                size,
            })
        })
    }

    /// Size of the .dol file. This is computed from the text and data sections: each section has
    /// an end, and the size of the .dol is considered to be the highest end.
    pub fn size(&self) -> u32 {
        self.text_sections()
            .chain(self.data_sections())
            .map(|sec| sec.offset + sec.size)
            .max()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Section<'a> {
    pub target: u32,
    pub content: &'a [u8],
}

/// A .dol executable.
#[derive(Debug, BinRead, BinWrite)]
#[brw(big)]
pub struct Dol {
    /// Header of the executable.
    #[brw(pad_size_to = HEADER_SIZE)]
    pub header: Header,
    /// Body of the executable.
    #[br(count = header.size() - HEADER_SIZE as u32)]
    pub body: Vec<u8>,
}

impl Dol {
    fn bytes(&self, offset: u32, size: u32) -> &[u8] {
        &self.body[offset as usize - HEADER_SIZE..][..size as usize]
    }

    pub fn text_sections(&self) -> impl Iterator<Item = Section<'_>> {
        self.header.text_sections().map(|info| {
            let content = self.bytes(info.offset, info.size);
            Section {
                target: info.target,
                content,
            }
        })
    }

    pub fn data_sections(&self) -> impl Iterator<Item = Section<'_>> {
        self.header.data_sections().map(|info| {
            let content = self.bytes(info.offset, info.size);
            Section {
                target: info.target,
                content,
            }
        })
    }

    pub fn entrypoint(&self) -> u32 {
        self.header.entry
    }
}

#[derive(Debug, Error)]
pub enum ElfToDolError {
    #[error(transparent)]
    Elf { source: elf::ParseError },
    #[error("elf has multiple .bss sections")]
    MultipleBss,
    #[error("elf has more than 7 .text sections")]
    TooManyTextSections,
    #[error("elf has more than 11 .data sections")]
    TooManyDataSections,
}

pub fn elf_to_dol(reader: impl Read + Seek) -> Result<Dol, ElfToDolError> {
    let mut elf = elf::ElfStream::<elf::endian::AnyEndian, _>::open_stream(reader)
        .context(ElfToDolCtx::Elf)?;

    let entry = elf.ehdr.e_entry;
    let segments = elf.segments().clone();
    let has_flag = |value, flag| value & flag != 0;

    let mut text = vec![];
    let mut data = vec![];
    let mut bss = None;
    for segment in segments {
        if !matches!(segment.p_type, elf::abi::PT_LOAD) || segment.p_memsz == 0 {
            continue;
        }

        let target = segment.p_vaddr;
        if segment.p_filesz == 0 {
            if bss.is_some() {
                return Err(ElfToDolError::MultipleBss);
            }

            bss = Some((target, segment.p_memsz));
            continue;
        }

        if has_flag(segment.p_flags, elf::abi::PF_X) {
            let bytes = elf
                .segment_data(&segment)
                .context(ElfToDolCtx::Elf)?
                .to_vec();

            text.push((target, bytes));
            continue;
        }

        if has_flag(segment.p_flags, elf::abi::PF_R) || has_flag(segment.p_flags, elf::abi::PF_W) {
            let bytes = elf
                .segment_data(&segment)
                .context(ElfToDolCtx::Elf)?
                .to_vec();

            data.push((target, bytes));
        }
    }

    if text.len() > 7 {
        return Err(ElfToDolError::TooManyTextSections);
    }

    if data.len() > 11 {
        return Err(ElfToDolError::TooManyDataSections);
    }

    let mut header = Header::default();
    let mut body = Vec::new();

    header.entry = entry as u32;

    for (i, (target, bytes)) in text.into_iter().enumerate() {
        header.text_offsets[i] = (HEADER_SIZE + body.len()) as u32;
        header.text_targets[i] = target as u32;
        header.text_sizes[i] = bytes.len() as u32;
        body.extend(bytes);
    }

    for (i, (target, bytes)) in data.into_iter().enumerate() {
        header.data_offsets[i] = (HEADER_SIZE + body.len()) as u32;
        header.data_targets[i] = target as u32;
        header.data_sizes[i] = bytes.len() as u32;
        body.extend(bytes);
    }

    if let Some((target, size)) = bss {
        header.bss_target = target as u32;
        header.bss_size = size as u32;
    }

    Ok(Dol { header, body })
}
