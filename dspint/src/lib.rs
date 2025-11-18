#![feature(cold_path)]

mod exec;

pub mod ins;

use crate::ins::{ExtensionOpcode, Opcode};
use bitos::{BitUtils, bitos, integer::u15};
use hemisphere::Primitive;
use hemisphere::system::{
    System,
    dspi::{DspDmaControl, DspDmaDirection, DspDmaTarget, Mailbox},
};
use strum::FromRepr;
use tinyvec::ArrayVec;
use util::boxed_array;

pub use ins::Ins;
use zerocopy::IntoBytes;

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

    pub fn set(&mut self, value: i64) {
        self.low = value as u16;
        self.mid1 = 0;
        self.mid2 = (value >> 16) as u16;
        self.high = (value >> 32) as u8;
    }
}

#[bitos(16)]
#[derive(Debug, Clone, Copy)]
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

impl Default for Status {
    fn default() -> Self {
        Self::from_bits(0)
            .with_interrupt_enable(true)
            .with_external_interrupt_enable(true)
    }
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

#[derive(Debug, Clone)]
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

impl Default for Registers {
    fn default() -> Self {
        Self {
            pc: Default::default(),
            addressing: Default::default(),
            indexing: Default::default(),
            wrapping: [0xFFFF; 4],
            call_stack: Default::default(),
            data_stack: Default::default(),
            loop_stack: Default::default(),
            loop_count: Default::default(),
            product: Default::default(),
            acc40: Default::default(),
            acc32: Default::default(),
            config: Default::default(),
            status: Default::default(),
        }
    }
}

impl Registers {
    pub fn get(&self, reg: Reg) -> u16 {
        let acc_saturate = |i: usize| {
            let ml = self.acc40[i].get() as i32 as i64;
            let hml = self.acc40[i].get();

            if self.status.sign_extend_to_40() && ml != hml {
                if hml >= 0 { 0x7FFF } else { 0x8000 }
            } else {
                self.acc40[i].mid
            }
        };

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
            Reg::Acc40Mid0 => acc_saturate(0),
            Reg::Acc40Mid1 => acc_saturate(1),
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

    pub fn set_saturate(&mut self, reg: Reg, value: u16) {
        let mut acc_saturate = |i: usize| {
            if !self.status.sign_extend_to_40() {
                self.acc40[i].mid = value;
                return;
            }

            self.acc40[i].low = 0;
            self.acc40[i].mid = value;
            self.acc40[i].high = if value.bit(15) { !0 } else { 0 };
        };

        match reg {
            Reg::Acc40Mid0 => acc_saturate(0),
            Reg::Acc40Mid1 => acc_saturate(1),
            Reg::LoopStack => (),
            _ => self.set(reg, value),
        }
    }
}

#[derive(Default)]
pub struct Accelerator {
    pub format: u16,
    pub gain: u16,
    pub pred_scale: u16,
    pub aram_start: u32,
    pub aram_end: u32,
    pub aram_curr: u32,
}

#[derive(Default)]
pub struct Interpreter {
    pub regs: Registers,
    pub mem: Memory,
    pub accel: Accelerator,
    pub loop_counter: Option<u16>,
    pub old_reset_high: bool,
}

type ExecFn = for<'a, 'b> fn(&'a mut Interpreter, &'b mut System, Ins);

static OPCODE_EXEC_LUT: [ExecFn; 1 << 8] = {
    fn nop(_: &mut Interpreter, _: &mut System, _: Ins) {}
    let mut lut = [nop as ExecFn; 1 << 8];

    lut[Opcode::Abs as usize] = Interpreter::abs as ExecFn;
    lut[Opcode::Add as usize] = Interpreter::add as ExecFn;
    lut[Opcode::Addarn as usize] = Interpreter::addarn as ExecFn;
    lut[Opcode::Addax as usize] = Interpreter::addax as ExecFn;
    lut[Opcode::Addaxl as usize] = Interpreter::addaxl as ExecFn;
    lut[Opcode::Addi as usize] = Interpreter::addi as ExecFn;
    lut[Opcode::Addis as usize] = Interpreter::addis as ExecFn;
    lut[Opcode::Addp as usize] = Interpreter::addp as ExecFn;
    lut[Opcode::Addpaxz as usize] = Interpreter::addpaxz as ExecFn;
    lut[Opcode::Addr as usize] = Interpreter::addr as ExecFn;
    lut[Opcode::Andc as usize] = Interpreter::andc as ExecFn;
    lut[Opcode::Andcf as usize] = Interpreter::andcf as ExecFn;
    lut[Opcode::Andf as usize] = Interpreter::andf as ExecFn;
    lut[Opcode::Andi as usize] = Interpreter::andi as ExecFn;
    lut[Opcode::Andr as usize] = Interpreter::andr as ExecFn;
    lut[Opcode::Asl as usize] = Interpreter::asl as ExecFn;
    lut[Opcode::Asr as usize] = Interpreter::asr as ExecFn;
    lut[Opcode::Asr16 as usize] = Interpreter::asr16 as ExecFn;
    lut[Opcode::Asrn as usize] = Interpreter::asrn as ExecFn;
    lut[Opcode::Asrnr as usize] = Interpreter::asrnr as ExecFn;
    lut[Opcode::Asrnrx as usize] = Interpreter::asrnrx as ExecFn;
    lut[Opcode::Bloop as usize] = Interpreter::bloop as ExecFn;
    lut[Opcode::Bloopi as usize] = Interpreter::bloopi as ExecFn;
    lut[Opcode::Call as usize] = Interpreter::call as ExecFn;
    lut[Opcode::Callr as usize] = Interpreter::callr as ExecFn;
    lut[Opcode::Clr as usize] = Interpreter::clr as ExecFn;
    lut[Opcode::Clr15 as usize] = Interpreter::clr15 as ExecFn;
    lut[Opcode::Clrl as usize] = Interpreter::clrl as ExecFn;
    lut[Opcode::Clrp as usize] = Interpreter::clrp as ExecFn;
    lut[Opcode::Cmp as usize] = Interpreter::cmp as ExecFn;
    lut[Opcode::Cmpaxh as usize] = Interpreter::cmpaxh as ExecFn;
    lut[Opcode::Cmpi as usize] = Interpreter::cmpi as ExecFn;
    lut[Opcode::Cmpis as usize] = Interpreter::cmpis as ExecFn;
    lut[Opcode::Dar as usize] = Interpreter::dar as ExecFn;
    lut[Opcode::Dec as usize] = Interpreter::dec as ExecFn;
    lut[Opcode::Decm as usize] = Interpreter::decm as ExecFn;
    lut[Opcode::Halt as usize] = Interpreter::halt as ExecFn;
    lut[Opcode::Iar as usize] = Interpreter::iar as ExecFn;
    lut[Opcode::If as usize] = Interpreter::ifcc as ExecFn;
    lut[Opcode::Ilrr as usize] = Interpreter::ilrr as ExecFn;
    lut[Opcode::Ilrrd as usize] = Interpreter::ilrrd as ExecFn;
    lut[Opcode::Ilrri as usize] = Interpreter::ilrri as ExecFn;
    lut[Opcode::Ilrrn as usize] = Interpreter::ilrrn as ExecFn;
    lut[Opcode::Inc as usize] = Interpreter::inc as ExecFn;
    lut[Opcode::Incm as usize] = Interpreter::incm as ExecFn;
    lut[Opcode::Jmp as usize] = Interpreter::jmp as ExecFn;
    lut[Opcode::Jr as usize] = Interpreter::jmpr as ExecFn;
    lut[Opcode::Loop as usize] = Interpreter::loop_ as ExecFn;
    lut[Opcode::Loopi as usize] = Interpreter::loopi as ExecFn;
    lut[Opcode::Lr as usize] = Interpreter::lr as ExecFn;
    lut[Opcode::Lri as usize] = Interpreter::lri as ExecFn;
    lut[Opcode::Lris as usize] = Interpreter::lris as ExecFn;
    lut[Opcode::Lrr as usize] = Interpreter::lrr as ExecFn;
    lut[Opcode::Lrrd as usize] = Interpreter::lrrd as ExecFn;
    lut[Opcode::Lrri as usize] = Interpreter::lrri as ExecFn;
    lut[Opcode::Lrrn as usize] = Interpreter::lrrn as ExecFn;
    lut[Opcode::Lrs as usize] = Interpreter::lrs as ExecFn;
    lut[Opcode::Lsl as usize] = Interpreter::lsl as ExecFn;
    lut[Opcode::Lsl16 as usize] = Interpreter::lsl16 as ExecFn;
    lut[Opcode::Lsr as usize] = Interpreter::lsr as ExecFn;
    lut[Opcode::Lsr16 as usize] = Interpreter::lsr16 as ExecFn;
    lut[Opcode::Lsrn as usize] = Interpreter::lsrn as ExecFn;
    lut[Opcode::Lsrnr as usize] = Interpreter::lsrnr as ExecFn;
    lut[Opcode::Lsrnrx as usize] = Interpreter::lsrnrx as ExecFn;
    lut[Opcode::M0 as usize] = Interpreter::m0 as ExecFn;
    lut[Opcode::M2 as usize] = Interpreter::m2 as ExecFn;
    lut[Opcode::Madd as usize] = Interpreter::madd as ExecFn;
    lut[Opcode::Maddc as usize] = Interpreter::maddc as ExecFn;
    lut[Opcode::Maddx as usize] = Interpreter::maddx as ExecFn;
    lut[Opcode::Mov as usize] = Interpreter::mov as ExecFn;
    lut[Opcode::Movax as usize] = Interpreter::movax as ExecFn;
    lut[Opcode::Movnp as usize] = Interpreter::movnp as ExecFn;
    lut[Opcode::Movp as usize] = Interpreter::movp as ExecFn;
    lut[Opcode::Movpz as usize] = Interpreter::movpz as ExecFn;
    lut[Opcode::Movr as usize] = Interpreter::movr as ExecFn;
    lut[Opcode::Mrr as usize] = Interpreter::mrr as ExecFn;
    lut[Opcode::Msub as usize] = Interpreter::msub as ExecFn;
    lut[Opcode::Msubc as usize] = Interpreter::msubc as ExecFn;
    lut[Opcode::Msubx as usize] = Interpreter::msubx as ExecFn;
    lut[Opcode::Mul as usize] = Interpreter::mul as ExecFn;
    lut[Opcode::Mulac as usize] = Interpreter::mulac as ExecFn;
    lut[Opcode::Mulaxh as usize] = Interpreter::mulaxh as ExecFn;
    lut[Opcode::Mulc as usize] = Interpreter::mulc as ExecFn;
    lut[Opcode::Mulcac as usize] = Interpreter::mulcac as ExecFn;
    lut[Opcode::Mulcmv as usize] = Interpreter::mulcmv as ExecFn;
    lut[Opcode::Mulcmvz as usize] = Interpreter::mulcmvz as ExecFn;
    lut[Opcode::Mulmv as usize] = Interpreter::mulmv as ExecFn;
    lut[Opcode::Mulmvz as usize] = Interpreter::mulmvz as ExecFn;
    lut[Opcode::Mulx as usize] = Interpreter::mulx as ExecFn;
    lut[Opcode::Mulxac as usize] = Interpreter::mulxac as ExecFn;
    lut[Opcode::Mulxmv as usize] = Interpreter::mulxmv as ExecFn;
    lut[Opcode::Mulxmvz as usize] = Interpreter::mulxmvz as ExecFn;
    lut[Opcode::Neg as usize] = Interpreter::neg as ExecFn;
    lut[Opcode::Not as usize] = Interpreter::not as ExecFn;
    lut[Opcode::Orc as usize] = Interpreter::orc as ExecFn;
    lut[Opcode::Ori as usize] = Interpreter::ori as ExecFn;
    lut[Opcode::Orr as usize] = Interpreter::orr as ExecFn;
    lut[Opcode::Ret as usize] = Interpreter::ret as ExecFn;
    lut[Opcode::Rti as usize] = Interpreter::rti as ExecFn;
    lut[Opcode::Sbclr as usize] = Interpreter::sbclr as ExecFn;
    lut[Opcode::Sbset as usize] = Interpreter::sbset as ExecFn;
    lut[Opcode::Set15 as usize] = Interpreter::set15 as ExecFn;
    lut[Opcode::Set16 as usize] = Interpreter::set16 as ExecFn;
    lut[Opcode::Set40 as usize] = Interpreter::set40 as ExecFn;
    lut[Opcode::Si as usize] = Interpreter::si as ExecFn;
    lut[Opcode::Sr as usize] = Interpreter::sr as ExecFn;
    lut[Opcode::Srr as usize] = Interpreter::srr as ExecFn;
    lut[Opcode::Srrd as usize] = Interpreter::srrd as ExecFn;
    lut[Opcode::Srri as usize] = Interpreter::srri as ExecFn;
    lut[Opcode::Srrn as usize] = Interpreter::srrn as ExecFn;
    lut[Opcode::Srs as usize] = Interpreter::srs as ExecFn;
    lut[Opcode::Srsh as usize] = Interpreter::srsh as ExecFn;
    lut[Opcode::Sub as usize] = Interpreter::sub as ExecFn;
    lut[Opcode::Subarn as usize] = Interpreter::subarn as ExecFn;
    lut[Opcode::Subax as usize] = Interpreter::subax as ExecFn;
    lut[Opcode::Subp as usize] = Interpreter::subp as ExecFn;
    lut[Opcode::Subr as usize] = Interpreter::subr as ExecFn;
    lut[Opcode::Tst as usize] = Interpreter::tst as ExecFn;
    lut[Opcode::Tstaxh as usize] = Interpreter::tstaxh as ExecFn;
    lut[Opcode::Tstprod as usize] = Interpreter::tstprod as ExecFn;
    lut[Opcode::Xorc as usize] = Interpreter::xorc as ExecFn;
    lut[Opcode::Xori as usize] = Interpreter::xori as ExecFn;
    lut[Opcode::Xorr as usize] = Interpreter::xorr as ExecFn;

    lut
};

impl Interpreter {
    fn raise_exception(&mut self, exception: Exception) {
        self.regs.call_stack.push(self.regs.pc);
        self.regs.data_stack.push(self.regs.status.to_bits());
        self.regs.pc = exception as u16 * 2;
    }

    fn check_external_interrupt(&mut self, sys: &mut System) {
        if sys.dsp.control.interrupt() && self.regs.status.external_interrupt_enable() {
            tracing::warn!("DSP external interrupt raised");

            sys.dsp.control.set_interrupt(false);
            self.raise_exception(Exception::Interrupt);
        }
    }

    fn check_stacks(&mut self) {
        if self
            .regs
            .loop_stack
            .last()
            .is_some_and(|v| *v == self.regs.pc)
        {
            let counter = self.regs.loop_count.last_mut().unwrap();
            *counter = counter.saturating_sub(1);

            if *counter == 0 {
                self.regs.call_stack.pop();
                self.regs.loop_stack.pop();
                self.regs.loop_count.pop();
            } else {
                let addr = *self.regs.call_stack.last().unwrap();
                self.regs.pc = addr;
            }
        }
    }

    /// Soft resets the DSP.
    pub fn reset(&mut self, sys: &mut System) {
        self.loop_counter = None;

        self.regs.wrapping = [0xFFFF; 4];
        self.regs.call_stack.clear();
        self.regs.data_stack.clear();
        self.regs.loop_stack.clear();
        self.regs.loop_count.clear();

        sys.dsp.dsp_mailbox = Mailbox::from_bits(0);
        sys.dsp.cpu_mailbox = Mailbox::from_bits(0);

        self.regs.pc = if sys.dsp.control.reset_high() {
            tracing::debug!("resetting at IROM (0x8000)");
            0x8000
        } else {
            tracing::debug!("resetting at IRAM (0x0000)");
            0x0000
        };
    }

    /// Checks for reset.
    pub fn check_reset(&mut self, sys: &mut System) {
        if sys.dsp.control.reset() || (sys.dsp.control.reset_high() != self.old_reset_high) {
            std::hint::cold_path();

            // DMA from main memory if resetting at low
            if !sys.dsp.control.reset_high() {
                tracing::debug!("DSP DMA stub from main memory");
                let data = sys.mem.ram[0x0100_0000..][..1024]
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]));

                for (word, data) in self.mem.iram[..512].iter_mut().zip(data) {
                    *word = data;
                }
            }

            tracing::debug!("DSP reset");
            self.reset(sys);
        }

        sys.dsp.control.set_reset(false);
        self.old_reset_high = sys.dsp.control.reset_high();
    }

    /// Performs the DSP DMA if the transfer is ongoing.
    pub fn do_dma(&mut self, sys: &mut System) {
        if sys.dsp.dsp_dma.control.transfer_ongoing() {
            std::hint::cold_path();

            let ram_base = sys.dsp.dsp_dma.ram_base.with_bits(26, 32, 0);
            let dsp_base = sys.dsp.dsp_dma.dsp_base;
            let length = sys.dsp.dsp_dma.length;

            let (target, direction) = (
                sys.dsp.dsp_dma.control.dsp_target(),
                sys.dsp.dsp_dma.control.direction(),
            );

            match (target, direction) {
                (DspDmaTarget::Dmem, DspDmaDirection::FromRamToDsp) => {
                    tracing::debug!(
                        "DSP DMA {length:04X} bytes from RAM {ram_base:08X} to DMEM {dsp_base:04X}",
                    );

                    for word in 0..(length / 2) {
                        let data = u16::read_be_bytes(
                            &sys.mem.ram[(ram_base + 2 * word as u32) as usize..],
                        );

                        self.write_dmem(sys, dsp_base + word, data);
                    }
                }
                (DspDmaTarget::Dmem, DspDmaDirection::FromDspToRam) => {
                    tracing::debug!(
                        "DSP DMA {length:04X} bytes from DMEM {dsp_base:04X} to RAM {ram_base:08X}"
                    );

                    for word in 0..(length / 2) {
                        let data = self.read_dmem(sys, dsp_base + word);
                        data.write_be_bytes(
                            &mut sys.mem.ram[(ram_base + 2 * word as u32) as usize..],
                        );
                    }
                }
                (DspDmaTarget::Imem, DspDmaDirection::FromRamToDsp) => {
                    tracing::info!(
                        "DSP DMA {length:04X} bytes from RAM {ram_base:08X} to IMEM {dsp_base:04X} (ucode)"
                    );

                    for word in 0..(length / 2) {
                        let data = u16::read_be_bytes(
                            &sys.mem.ram[(ram_base + 2 * word as u32) as usize..],
                        );

                        self.write_imem(dsp_base + word, data);
                    }
                }
                (DspDmaTarget::Imem, DspDmaDirection::FromDspToRam) => {
                    todo!()
                }
            };

            sys.dsp.dsp_dma.length = 0;
            sys.dsp.dsp_dma.control.set_transfer_ongoing(false);
            sys.dsp.control.set_dsp_dma_ongoing(false);
        }
    }

    pub fn read_mmio(&mut self, sys: &mut System, offset: u8) -> u16 {
        match offset {
            // DMA
            0xC9 => sys.dsp.dsp_dma.control.to_bits(),
            0xCB => sys.dsp.dsp_dma.length,
            0xCD => sys.dsp.dsp_dma.dsp_base,
            0xCE => (sys.dsp.dsp_dma.ram_base >> 16) as u16,
            0xCF => sys.dsp.dsp_dma.ram_base as u16,

            // Accelerator
            0xD3 => {
                let value = u16::read_be_bytes(
                    sys.mem.aram[self.accel.aram_curr.with_bit(31, false) as usize..].as_bytes(),
                );

                tracing::debug!(
                    "accelerator reading 0x{value:04X} from ARAM 0x{:08X} (wraps at 0x{:08X})",
                    self.accel.aram_curr,
                    self.accel.aram_end
                );

                self.accel.aram_curr += 1;
                value
            }
            0xD4 => self.accel.aram_start.bits(16, 32) as u16,
            0xD5 => self.accel.aram_start.bits(0, 16) as u16,
            0xD6 => self.accel.aram_end.bits(16, 32) as u16,
            0xD7 => self.accel.aram_end.bits(0, 16) as u16,
            0xD8 => self.accel.aram_curr.bits(16, 32) as u16,
            0xD9 => self.accel.aram_curr.bits(0, 16) as u16,
            0xDA => self.accel.pred_scale,
            0xDB => 0,
            0xDC => 0,
            0xDD => 0,
            0xDE => self.accel.gain,

            // Mailboxes
            0xFC => sys.dsp.dsp_mailbox.high_and_status(),
            0xFD => sys.dsp.dsp_mailbox.low(),
            0xFE => sys.dsp.cpu_mailbox.high_and_status(),
            0xFF => {
                if sys.dsp.cpu_mailbox.status() {
                    tracing::debug!(
                        "received from CPU mailbox: 0x{:08X}",
                        sys.dsp.cpu_mailbox.data().value()
                    );
                    sys.dsp.cpu_mailbox.set_status(false);
                }

                sys.dsp.cpu_mailbox.low()
            }
            _ => unimplemented!("read from {offset:02X}"),
        }
    }

    pub fn write_mmio(&mut self, sys: &mut System, offset: u8, value: u16) {
        match offset {
            // Coefficients
            0xA0..=0xAF => (),

            // DMA
            0xC9 => sys.dsp.dsp_dma.control = DspDmaControl::from_bits(value),
            0xCB => {
                sys.dsp.dsp_dma.length = value;
                sys.dsp.dsp_dma.control.set_transfer_ongoing(true);
                sys.dsp.control.set_dsp_dma_ongoing(true);
            }
            0xCD => sys.dsp.dsp_dma.dsp_base = value,
            0xCE => {
                sys.dsp.dsp_dma.ram_base = sys.dsp.dsp_dma.ram_base.with_bits(16, 32, value as u32)
            }
            0xCF => {
                sys.dsp.dsp_dma.ram_base = sys.dsp.dsp_dma.ram_base.with_bits(0, 16, value as u32)
            }

            // Interrupt
            0xFB => {
                if value > 0 {
                    sys.dsp.control.set_dsp_interrupt(true);
                }
            }

            // Accelerator
            0xD1 => self.accel.format = value,
            0xD3 => {
                tracing::debug!(
                    "accelerator writing 0x{value:04X} to ARAM 0x{:08X} (wraps at 0x{:08X})",
                    self.accel.aram_curr,
                    self.accel.aram_end
                );

                value.write_be_bytes(
                    sys.mem.aram[self.accel.aram_curr.with_bit(31, false) as usize..]
                        .as_mut_bytes(),
                );
                self.accel.aram_curr += 1;
            }
            0xD4 => self.accel.aram_start = self.accel.aram_start.with_bits(16, 32, value as u32),
            0xD5 => self.accel.aram_start = self.accel.aram_start.with_bits(0, 16, value as u32),
            0xD6 => self.accel.aram_end = self.accel.aram_end.with_bits(16, 32, value as u32),
            0xD7 => self.accel.aram_end = self.accel.aram_end.with_bits(0, 16, value as u32),
            0xD8 => self.accel.aram_curr = self.accel.aram_curr.with_bits(16, 32, value as u32),
            0xD9 => self.accel.aram_curr = self.accel.aram_curr.with_bits(0, 16, value as u32),
            0xDA => self.accel.pred_scale = value,
            0xDB => (),
            0xDC => (),
            0xDE => self.accel.gain = value,

            // Mailboxes
            0xFC => {
                sys.dsp.dsp_mailbox.set_high(u15::new(value));
            }
            0xFD => {
                sys.dsp.dsp_mailbox.set_low(value);
                sys.dsp.dsp_mailbox.set_status(true);
            }
            _ => unimplemented!("write to {offset:02X}"),
        }
    }

    /// Reads from data memory.
    pub fn read_dmem(&mut self, sys: &mut System, addr: u16) -> u16 {
        let value = match addr {
            0x0000..0x1000 => self.mem.dram[addr as usize],
            0x1000..0x1800 => self.mem.coef[addr as usize - 0x1000],
            0xFF00.. => self.read_mmio(sys, addr as u8),
            _ => 0,
        };

        value
    }

    /// Writes to data memory.
    pub fn write_dmem(&mut self, sys: &mut System, addr: u16, value: u16) {
        match addr {
            0x0000..0x1000 => self.mem.dram[addr as usize] = value,
            0x1000..0x1800 => tracing::warn!("writing to coefficient data"),
            0xFF00.. => self.write_mmio(sys, addr as u8, value),
            _ => (),
        }
    }

    /// Reads from instruction memory.
    #[inline(always)]
    pub fn read_imem(&mut self, addr: u16) -> u16 {
        match addr {
            0x0000..0x1000 => self.mem.iram[addr as usize],
            0x8000..0x9000 => self.mem.irom[addr as usize - 0x8000],
            _ => 0,
        }
    }

    /// Writes to instruction memory.
    #[inline(always)]
    pub fn write_imem(&mut self, addr: u16, value: u16) {
        match addr {
            0x0000..0x1000 => self.mem.iram[addr as usize] = value,
            _ => (),
        }
    }

    fn is_waiting_for_mail_inner(&mut self, offset: i16) -> bool {
        let start = self.regs.pc.wrapping_add_signed(offset);
        let pattern_a = [
            // lrs   $ACM0, @cmbh
            0b0010_0110_1111_1110,
            // andcf $ACM0, #0x8000
            0b0000_0010_1100_0000,
            0x8000,
            // jlnz	 start
            0b0000_0010_1001_1100,
            start,
        ];

        let pattern_b = [
            // lrs   $ACM1, @cmbh
            0b0010_0111_1111_1110,
            // andcf $ACM1, #0x8000
            0b0000_0011_1100_0000,
            0x8000,
            // jlnz	 start
            0b0000_0010_1001_1100,
            start,
        ];

        let current = [
            self.read_imem(start),
            self.read_imem(start.wrapping_add(1)),
            self.read_imem(start.wrapping_add(2)),
            self.read_imem(start.wrapping_add(3)),
            self.read_imem(start.wrapping_add(4)),
        ];

        current == pattern_a || current == pattern_b
    }

    #[inline(always)]
    pub fn is_waiting_for_mail(&mut self) -> bool {
        self.is_waiting_for_mail_inner(0)
            || self.is_waiting_for_mail_inner(-1)
            || self.is_waiting_for_mail_inner(-3)
    }

    pub fn step(&mut self, sys: &mut System) {
        if sys.dsp.control.halt() {
            std::hint::cold_path();
            return;
        }

        self.check_stacks();
        if self.loop_counter.is_none() {
            self.check_external_interrupt(sys);
        }

        // fetch
        let mut ins = Ins::new(self.read_imem(self.regs.pc));
        let decoded = ins.decoded();

        let extra = decoded
            .needs_extra
            .then_some(self.read_imem(self.regs.pc.wrapping_add(1)));

        let ins_len = if let Some(extra) = extra {
            ins.extra = extra;
            2
        } else {
            1
        };

        // execute
        let regs_previous = if decoded.extension.is_some() {
            Some(self.regs.clone())
        } else {
            None
        };

        OPCODE_EXEC_LUT[decoded.opcode as usize](self, sys, ins);

        if let Some(extension) = decoded.extension {
            let regs_previous = regs_previous.unwrap();
            match extension {
                ExtensionOpcode::Dr => self.ext_dr(sys, ins, &regs_previous),
                ExtensionOpcode::Ir => self.ext_ir(sys, ins, &regs_previous),
                ExtensionOpcode::L => self.ext_l(sys, ins, &regs_previous),
                ExtensionOpcode::Ld => self.ext_ld(sys, ins, &regs_previous),
                ExtensionOpcode::Ldm => self.ext_ldm(sys, ins, &regs_previous),
                ExtensionOpcode::Ldn => self.ext_ldn(sys, ins, &regs_previous),
                ExtensionOpcode::Ldnm => self.ext_ldnm(sys, ins, &regs_previous),
                ExtensionOpcode::Ln => self.ext_ln(sys, ins, &regs_previous),
                ExtensionOpcode::Ls => self.ext_ls(sys, ins, &regs_previous),
                ExtensionOpcode::Lsm => self.ext_lsm(sys, ins, &regs_previous),
                ExtensionOpcode::Lsn => self.ext_lsn(sys, ins, &regs_previous),
                ExtensionOpcode::Lsnm => self.ext_lsnm(sys, ins, &regs_previous),
                ExtensionOpcode::Mv => self.ext_mv(sys, ins, &regs_previous),
                ExtensionOpcode::Nop => (),
                ExtensionOpcode::Nr => self.ext_nr(sys, ins, &regs_previous),
                ExtensionOpcode::S => self.ext_s(sys, ins, &regs_previous),
                ExtensionOpcode::Sl => self.ext_sl(sys, ins, &regs_previous),
                ExtensionOpcode::Slm => self.ext_slm(sys, ins, &regs_previous),
                ExtensionOpcode::Sln => self.ext_sln(sys, ins, &regs_previous),
                ExtensionOpcode::Slnm => self.ext_slnm(sys, ins, &regs_previous),
                ExtensionOpcode::Sn => self.ext_sn(sys, ins, &regs_previous),
                ExtensionOpcode::Illegal => panic!("illegal extension opcode"),
            }
        }

        if let Some(loop_counter) = &mut self.loop_counter {
            if *loop_counter == 0 {
                std::hint::cold_path();
                self.loop_counter = None;
                self.regs.pc += 1;
            } else {
                *loop_counter -= 1;
            }
        } else {
            self.regs.pc = self.regs.pc.wrapping_add(ins_len);
        }
    }
}
