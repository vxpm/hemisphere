use binrw::{BinRead, BinWrite, binread};

#[derive(Debug, BinRead, BinWrite)]
#[brw(big, magic = 1u8)]
pub struct Root {
    #[br(parse_with = binrw::helpers::read_u24)]
    #[bw(write_with = binrw::helpers::write_u24)]
    pub name_offset: u32,
    /// Entry count in this filesystem, including root.
    #[brw(pad_before = 4)]
    pub entry_count: u32,
}

#[derive(Debug, BinRead, BinWrite)]
#[brw(stream = s, big)]
pub struct FileEntry {
    /// Offset of this file entry
    #[br(calc = s.stream_position().unwrap() as u32)]
    #[bw(ignore)]
    pub offset: u32,
    /// Offset of the name of this file relative to the strings table
    #[br(parse_with = binrw::helpers::read_u24)]
    #[bw(write_with = binrw::helpers::write_u24)]
    pub name_offset: u32,
    /// Offset of the data of this file
    pub data_offset: u32,
    /// Length of the data of this file
    pub data_length: u32,
}

#[derive(Debug, BinRead, BinWrite)]
#[brw(stream = s, big)]
pub struct DirectoryEntry {
    /// Offset of this directory entry
    #[br(calc = s.stream_position().unwrap() as u32)]
    #[bw(ignore)]
    pub offset: u32,
    /// Offset of the name of this directory relative to the strings table
    #[br(parse_with = binrw::helpers::read_u24)]
    #[bw(write_with = binrw::helpers::write_u24)]
    pub name_offset: u32,
    pub parent_index: u32,
    pub end_index: u32,
}

#[derive(Debug, BinRead)]
#[br(big)]
pub enum Entry {
    #[brw(magic(0u8))]
    File(FileEntry),
    #[brw(magic(1u8))]
    Directory(DirectoryEntry),
}

#[binread]
#[derive(Debug)]
#[brw(big, stream = s)]
pub struct FileSystem {
    pub root: Root,
    #[br(calc = s.stream_position().unwrap() as u32 + (root.entry_count - 1) * 0xC)]
    pub strings_offset: u32,
    #[br(count = root.entry_count - 1)]
    pub entries: Vec<Entry>,
}
