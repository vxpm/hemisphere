use binrw::{BinRead, BinWrite, NullString};

#[derive(Debug, BinRead, BinWrite)]
#[brw(big)]
pub struct Header {
    #[brw(pad_size_to = 0x10)]
    #[brw(assert(version.len() <= 0x10))]
    pub version: NullString,
    pub entrypoint: u32,
    pub size: u32,
    pub trailer_size: u32,
}

// An apploader program in a .iso.
#[derive(Debug, BinRead, BinWrite)]
#[brw(big)]
pub struct Apploader {
    pub header: Header,
    #[brw(pad_before = 0x4)]
    #[br(count = header.size)]
    pub body: Vec<u8>,
}
