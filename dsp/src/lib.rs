mod exec;

pub mod ins;

use bitos::{BitUtils, bitos};
use common::util::boxed_array;
use strum::FromRepr;
use tinyvec::ArrayVec;

pub use ins::Ins;

use crate::ins::{ExtensionOpcode, Opcode};

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
pub struct Acc40 {
    pub low: u16,
    pub mid: u16,
    pub high: u8,
}

impl Acc40 {
    const MIN: i64 = (1 << 63) >> 24;

    pub fn from(value: i64) -> Self {
        Self {
            low: value.bits(0, 16) as u16,
            mid: value.bits(16, 32) as u16,
            high: value.bits(32, 40) as u8,
        }
    }

    pub fn get(&self) -> i64 {
        let bits = 0
            .with_bits(0, 16, self.low as i64)
            .with_bits(16, 32, self.mid as i64)
            .with_bits(32, 40, self.high as i64);

        (bits << 24) >> 24
    }

    pub fn set(&mut self, value: i64) -> i64 {
        *self = Self::from(value);
        self.get()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Product {
    pub low: u16,
    pub mid1: u16,
    pub mid2: u16,
    pub high: u8,
}

impl Product {
    pub fn get(&self) -> (bool, bool, i64) {
        let (sum, carry) = self.mid1.overflowing_add(self.mid2);
        let (c_high, carry) = self.high.overflowing_add(carry as u8);
        let overflow = self.high as i8 >= 0 && ((c_high as i8) < 0);

        let bits = 0
            .with_bits(0, 16, self.low as i64)
            .with_bits(16, 32, sum as i64)
            .with_bits(32, 40, c_high as i64);

        let value = (bits << 24) >> 24;

        (carry, overflow, value)
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
#[repr(u8)]
pub enum Reg {
    Addr0,
    Addr1,
    Addr2,
    Addr3,
    Index0,
    Index1,
    Index2,
    Index3,
    Wrap0,
    Wrap1,
    Wrap2,
    Wrap3,
    CallStack,
    DataStack,
    LoopStack,
    LoopCount,
    Acc40High0,
    Acc40High1,
    Config,
    Status,
    ProdLow,
    ProdMid1,
    ProdHigh,
    ProdMid2,
    Acc32Low0,
    Acc32Low1,
    Acc32High0,
    Acc32High1,
    Acc40Low0,
    Acc40Low1,
    Acc40Mid0,
    Acc40Mid1,
}

impl Reg {
    pub fn new(index: u8) -> Self {
        Self::from_repr(index).unwrap()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Registers {
    pub pc: u16,
    pub addressing: [u16; 4],
    pub indexing: [u16; 4],
    pub wrapping: [u16; 4],
    pub call_stack: ArrayVec<[u16; 8]>,
    pub data_stack: ArrayVec<[u16; 4]>,
    pub loop_stack: ArrayVec<[u16; 4]>,
    pub loop_count: ArrayVec<[u16; 4]>,
    pub product: Product,
    pub acc40: [Acc40; 2],
    pub acc32: [i32; 2],
    pub config: u8,
    pub status: Status,
}

impl Registers {
    pub fn get(&self, reg: Reg) -> u16 {
        match reg {
            Reg::Addr0 => self.addressing[0],
            Reg::Addr1 => self.addressing[1],
            Reg::Addr2 => self.addressing[2],
            Reg::Addr3 => self.addressing[3],
            Reg::Index0 => self.indexing[0],
            Reg::Index1 => self.indexing[1],
            Reg::Index2 => self.indexing[2],
            Reg::Index3 => self.indexing[3],
            Reg::Wrap0 => self.wrapping[0],
            Reg::Wrap1 => self.wrapping[1],
            Reg::Wrap2 => self.wrapping[2],
            Reg::Wrap3 => self.wrapping[3],
            Reg::CallStack => self.call_stack.last().copied().unwrap_or_default(),
            Reg::DataStack => self.data_stack.last().copied().unwrap_or_default(),
            Reg::LoopStack => self.loop_stack.last().copied().unwrap_or_default(),
            Reg::LoopCount => self.loop_count.last().copied().unwrap_or_default(),
            Reg::Acc40High0 => self.acc40[0].high as i8 as i16 as u16,
            Reg::Acc40High1 => self.acc40[1].high as i8 as i16 as u16,
            Reg::Config => self.config as u16,
            Reg::Status => self.status.to_bits(),
            Reg::ProdLow => self.product.low,
            Reg::ProdMid1 => self.product.mid1,
            Reg::ProdHigh => self.product.high as u16,
            Reg::ProdMid2 => self.product.mid2,
            Reg::Acc32Low0 => self.acc32[0].bits(0, 16) as u16,
            Reg::Acc32Low1 => self.acc32[1].bits(0, 16) as u16,
            Reg::Acc32High0 => self.acc32[0].bits(16, 32) as u16,
            Reg::Acc32High1 => self.acc32[1].bits(16, 32) as u16,
            Reg::Acc40Low0 => self.acc40[0].low,
            Reg::Acc40Low1 => self.acc40[1].low,
            Reg::Acc40Mid0 => self.acc40[0].mid,
            Reg::Acc40Mid1 => self.acc40[1].mid,
        }
    }

    pub fn set(&mut self, reg: Reg, value: u16) {
        match reg {
            Reg::Addr0 => self.addressing[0] = value,
            Reg::Addr1 => self.addressing[1] = value,
            Reg::Addr2 => self.addressing[2] = value,
            Reg::Addr3 => self.addressing[3] = value,
            Reg::Index0 => self.indexing[0] = value,
            Reg::Index1 => self.indexing[1] = value,
            Reg::Index2 => self.indexing[2] = value,
            Reg::Index3 => self.indexing[3] = value,
            Reg::Wrap0 => self.wrapping[0] = value,
            Reg::Wrap1 => self.wrapping[1] = value,
            Reg::Wrap2 => self.wrapping[2] = value,
            Reg::Wrap3 => self.wrapping[3] = value,
            Reg::CallStack => self.call_stack.push(value),
            Reg::DataStack => self.data_stack.push(value),
            Reg::LoopStack => self.loop_stack.push(value),
            Reg::LoopCount => self.loop_count.push(value),
            Reg::Acc40High0 => self.acc40[0].high = value as u8,
            Reg::Acc40High1 => self.acc40[1].high = value as u8,
            Reg::Config => self.config = value as u8,
            Reg::Status => self.status = Status::from_bits(value.with_bit(8, false)),
            Reg::ProdLow => self.product.low = value,
            Reg::ProdMid1 => self.product.mid1 = value,
            Reg::ProdHigh => self.product.high = value as u8,
            Reg::ProdMid2 => self.product.mid2 = value,
            Reg::Acc32Low0 => self.acc32[0] = self.acc32[0].with_bits(0, 16, value as i32),
            Reg::Acc32Low1 => self.acc32[1] = self.acc32[1].with_bits(0, 16, value as i32),
            Reg::Acc32High0 => self.acc32[0] = self.acc32[0].with_bits(16, 32, value as i32),
            Reg::Acc32High1 => self.acc32[1] = self.acc32[1].with_bits(16, 32, value as i32),
            Reg::Acc40Low0 => self.acc40[0].low = value,
            Reg::Acc40Low1 => self.acc40[1].low = value,
            Reg::Acc40Mid0 => self.acc40[0].mid = value,
            Reg::Acc40Mid1 => self.acc40[1].mid = value,
        }
    }
}

#[derive(Default)]
pub struct Control {
    pub halt: bool,
}

#[derive(Default)]
pub struct Dsp {
    pub regs: Registers,
    pub memory: Memory,
    pub control: Control,
}

impl Dsp {
    fn check_stacks(&mut self) {
        if self
            .regs
            .loop_stack
            .last()
            .is_some_and(|v| *v == self.regs.pc)
        {
            let counter = self.regs.loop_count.last_mut().unwrap();
            *counter -= 1;

            if *counter == 0 {
                self.regs.call_stack.pop();
                self.regs.loop_stack.pop();
                self.regs.loop_count.pop();
            } else {
                let offset = *self.regs.call_stack.last().unwrap();
                self.regs.pc = self.regs.pc.wrapping_add(offset);
            }
        }
    }

    pub fn step(&mut self) {
        self.check_stacks();

        // fetch
        let mut ins = Ins::new(self.memory.iram[self.regs.pc as usize]);
        let opcode = ins.opcode();

        let extra = opcode
            .needs_extra()
            .then_some(self.memory.iram[self.regs.pc as usize + 1]);

        if let Some(extra) = extra {
            ins.extra = extra;
        }

        // execute
        // let regs_previous = self.regs.clone();
        match ins.opcode() {
            Opcode::Nop => (),
            Opcode::Abs => self.abs(ins),
            Opcode::Add => self.add(ins),
            Opcode::Addarn => self.addarn(ins),
            Opcode::Addax => self.addax(ins),
            Opcode::Addaxl => self.addaxl(ins),
            Opcode::Addi => self.addi(ins),
            Opcode::Addis => self.addis(ins),
            Opcode::Addp => self.addp(ins),
            Opcode::Addpaxz => self.addpaxz(ins),
            Opcode::Addr => self.addr(ins),
            Opcode::Andc => self.andc(ins),
            Opcode::Andcf => self.andcf(ins),
            Opcode::Andf => self.andf(ins),
            Opcode::Andi => self.andi(ins),
            Opcode::Andr => self.andr(ins),
            Opcode::Asl => self.asl(ins),
            Opcode::Asr => self.asr(ins),
            Opcode::Asrn => self.asrn(ins),
            Opcode::Asrnr => self.asrnr(ins),
            Opcode::Asrnrx => self.asrnrx(ins),
            Opcode::Asr16 => self.asr16(ins),
            Opcode::Clr15 => self.clr15(ins),
            Opcode::Clr => self.clr(ins),
            Opcode::Clrl => self.clrl(ins),
            Opcode::Clrp => self.clrp(ins),
            Opcode::Cmp => self.cmp(ins),
            Opcode::Cmpaxh => self.cmpaxh(ins),
            Opcode::Cmpi => self.cmpi(ins),
            Opcode::Cmpis => self.cmpis(ins),
            Opcode::Halt => self.halt(ins),
            _ => (),
        }

        if opcode.has_extension() {
            let extension = ins.extension_opcode();
            match extension {
                ExtensionOpcode::Nop => (),
                _ => todo!("extension op {extension:?}"),
            }
        }

        self.regs.pc += if extra.is_some() { 2 } else { 1 };
    }
}
