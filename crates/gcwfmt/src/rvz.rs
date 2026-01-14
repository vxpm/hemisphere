//! A `.rvz` file is a file format designed to store the same data as `.iso` files in a
//! space-efficient manner.

use binrw::{BinRead, BinWrite};

type Sha1Hash = [u8; 20];

/// The header of a .rvz file.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big, magic = b"WIA\x01")]
pub struct HeaderInner {
    pub version: u32,
    pub compatible_version: u32,
    pub disk_size: u32,
    pub disk_sha1: Sha1Hash,
    pub iso_size: u32,
    pub rvz_size: u32,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big)]
pub struct Header {
    pub inner: HeaderInner,
    pub hash: Sha1Hash,
}

// #[derive(Debug, Clone, BinRead, BinWrite)]
// pub struct Rvz {
//     pub header: Header,
//     pub console: Console,
// }
