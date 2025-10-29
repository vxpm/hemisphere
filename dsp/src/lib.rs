pub mod ins;

use bitos::{bitos, integer::u40};
use common::util::boxed_array;
use tinyvec::ArrayVec;

const IRAM_LEN: usize = 0x1000;
const IROM_LEN: usize = 0x1000;
const DRAM_LEN: usize = 0x1000;
const COEF_LEN: usize = 0x0800;

pub struct Memory {
    pub iram: Box<[u16; IRAM_LEN]>,
    pub irom: Box<[u16; IROM_LEN]>,
    pub dram: Box<[u16; DRAM_LEN]>,
    pub coef: Box<[u16; COEF_LEN]>,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            iram: boxed_array(0),
            irom: boxed_array(0),
            dram: boxed_array(0),
            coef: boxed_array(0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exception {
    Reset = 0,
    StackOverflow = 1,
    Unknown0 = 2,
    AccelRawReadOverflow = 3,
    AccelRawWriteOverflow = 4,
    AccelSampleReadOverflow = 5,
    Unknown1 = 6,
    Interrupt = 7,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Product {
    pub low: u16,
    pub mid0: u16,
    pub mid1: u16,
    pub high: u8,
}

#[bitos(16)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Status {
    #[bits(0)]
    pub carry: bool,
    #[bits(1)]
    pub overflow: bool,
    #[bits(2)]
    pub arithmetic_zero: bool,
    #[bits(3)]
    pub sign: bool,
    #[bits(4)]
    pub above_s32: bool,
    #[bits(5)]
    pub top_two_bits_eq: bool,
    #[bits(6)]
    pub logic_zero: bool,
    #[bits(7)]
    pub overflow_fused: bool,
    #[bits(9)]
    pub interrupt_enable: bool,
    #[bits(11)]
    pub external_interrupt_enable: bool,
    #[bits(13)]
    pub dont_double_result: bool,
    #[bits(14)]
    pub sign_extend_to_40: bool,
    #[bits(15)]
    pub unsigned_mul: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Registers {
    pub addressing: [u16; 4],
    pub indexing: [u16; 4],
    pub wrapping: [u16; 4],
    pub call_stack: ArrayVec<[u16; 8]>,
    pub data_stack: ArrayVec<[u16; 4]>,
    pub loop_stack: ArrayVec<[u16; 4]>,
    pub loop_count: ArrayVec<[u16; 4]>,
    pub product: Product,
    pub acc40: [u40; 2],
    pub acc32: [u32; 2],
    pub config: u8,
    pub status: Status,
}

#[derive(Default)]
pub struct Dsp {
    pub memory: Memory,
    pub regs: Registers,
}
