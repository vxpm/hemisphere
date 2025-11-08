use binrw::helpers::until_eof;
use binrw::io::BufReader;
use binrw::{BinRead, binread};
use std::path::Path;

#[derive(BinRead)]
#[br(import(instructions: u16), little)]
pub struct TestCase {
    #[br(count = instructions)]
    pub instructions: Vec<u16>,
    pub expected: [u16; 31],
    pub initial: [u16; 31],
}

fn regs(arr: [u16; 31]) -> dsp::Registers {
    let mut regs = dsp::Registers::default();
    for i in 0..31 {
        // reg 18 is not in the data
        let reg_index = if i < 18 { i } else { i + 1 };

        // reg 14 is skipped, it can't be written to by LIS
        if reg_index == 14 {
            continue;
        }

        regs.set(dsp::Reg::new(reg_index), arr[i as usize]);
    }

    regs
}

impl TestCase {
    pub fn expected_regs(&self) -> dsp::Registers {
        regs(self.expected)
    }

    pub fn initial_regs(&self) -> dsp::Registers {
        regs(self.initial)
    }
}

#[binread]
#[br(little)]
pub struct TestFile {
    #[br(temp)]
    case_length: u16,
    #[br(assert(case_length % 2 == 0))]
    #[br(parse_with = until_eof, args(case_length / 2))]
    pub cases: Vec<TestCase>,
}

impl TestFile {
    pub fn open(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let mut buffer = BufReader::new(std::fs::File::open(path).unwrap());
        Self::read(&mut buffer).unwrap()
    }
}
