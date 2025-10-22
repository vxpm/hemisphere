use binrw::{BinRead, BinWrite};

#[derive(Debug, BinRead, BinWrite)]
#[brw(big, magic = 1u8)]
pub struct Root {
    #[br(parse_with = binrw::helpers::read_u24)]
    #[bw(write_with = binrw::helpers::write_u24)]
    pub name_offset: u32,
    pub parent_offset: u32,
    pub num_entries: u32,
}

#[derive(Debug, BinRead, BinWrite)]
#[brw(big)]
pub enum Entry {
    #[brw(magic(0u8))]
    File {
        #[br(parse_with = binrw::helpers::read_u24)]
        #[bw(write_with = binrw::helpers::write_u24)]
        name_offset: u32,
        data_offset: u32,
        length: u32,
    },
    #[brw(magic(1u8))]
    Directory {
        #[br(parse_with = binrw::helpers::read_u24)]
        #[bw(write_with = binrw::helpers::write_u24)]
        name_offset: u32,
        parent_offset: u32,
        next_offset: u32,
    },
}
