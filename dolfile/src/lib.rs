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
    pub bss_offset: u32,
    /// Size of the bss section.
    pub bss_size: u32,
    /// Entrypoint of the executable.
    pub entry: u32,
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
    pub fn text_sections(&self) -> impl Iterator<Item = Section<'_>> {
        (0..7).filter_map(|i| {
            let offset = self.header.text_offsets[i];
            if offset == 0 {
                return None;
            }

            let size = self.header.text_sizes[i];
            let content = &self.body[offset as usize - HEADER_SIZE..][..size as usize];

            Some(Section {
                target: self.header.text_targets[i],
                content,
            })
        })
    }

    pub fn data_sections(&self) -> impl Iterator<Item = Section<'_>> {
        (0..11).filter_map(|i| {
            let offset = self.header.data_offsets[i];
            if offset == 0 {
                return None;
            }

            let size = self.header.data_sizes[i];
            let content = &self.body[offset as usize - HEADER_SIZE..][..size as usize];

            Some(Section {
                target: self.header.data_targets[i],
                content,
            })
        })
    }

    pub fn bss_data(&self) -> &[u8] {
        let offset = self.header.bss_offset;
        let size = self.header.bss_size;

        &self.body[offset as usize - HEADER_SIZE..][..size as usize]
    }

    pub fn entrypoint(&self) -> u32 {
        self.header.entry
    }
}
