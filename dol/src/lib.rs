pub use binrw;
use binrw::{BinRead, BinWrite, helpers::until_eof};

const HEADER_SIZE: usize = 0x100;

// The header of a .dol file.
#[derive(Debug, BinRead, BinWrite)]
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
    #[br(pad_size_to = HEADER_SIZE)]
    pub header: Header,
    /// Body of the executable.
    #[br(parse_with = until_eof)]
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
