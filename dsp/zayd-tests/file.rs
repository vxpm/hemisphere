use binrw::BinRead;
use binrw::helpers::until_eof;
use binrw::io::BufReader;
use std::path::Path;

#[derive(BinRead)]
#[br(import(instructions: u16), little)]
pub struct TestCase {
    #[br(count = instructions)]
    pub instructions: Vec<u8>,
    pub expected: [u16; 31],
    pub initial: [u16; 31],
}

#[derive(BinRead)]
#[br(little)]
pub struct TestFile {
    pub case_length: u16,
    #[br(parse_with = until_eof, args(case_length))]
    pub cases: Vec<TestCase>,
}

impl TestFile {
    pub fn open(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let mut buffer = BufReader::new(std::fs::File::open(path).unwrap());
        Self::read(&mut buffer).unwrap()
    }
}
