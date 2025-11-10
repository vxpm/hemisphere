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

            let ram_base = sys.dsp.dsp_dma.ram_base & 0xFF_FFFF;
            let dsp_base = sys.dsp.dsp_dma.dsp_base;
            let length = sys.dsp.dsp_dma.length;

            let (target, direction) = (
                sys.dsp.dsp_dma.control.dsp_target(),
                sys.dsp.dsp_dma.control.direction(),
            );

            match (target, direction) {
                (DspDmaTarget::Dmem, DspDmaDirection::FromRamToDsp) => {
                    tracing::debug!(
                        "DSP DMA {length:04X} bytes from RAM {ram_base:08X} to DMEM {dsp_base:04X}"
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
            0xD4 => self.accel.aram_end = self.accel.aram_start.with_bits(16, 32, value as u32),
            0xD5 => self.accel.aram_end = self.accel.aram_start.with_bits(0, 16, value as u32),
            0xD6 => self.accel.aram_end = self.accel.aram_end.with_bits(16, 32, value as u32),
            0xD7 => self.accel.aram_end = self.accel.aram_end.with_bits(0, 16, value as u32),
            0xD8 => self.accel.aram_curr = self.accel.aram_curr.with_bits(16, 32, value as u32),
            0xD9 => self.accel.aram_curr = self.accel.aram_curr.with_bits(0, 16, value as u32),

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
            0x1000..0x1800 => self.mem.coef[addr as usize - 0x1000] = value,
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
        let opcode = ins.opcode();

        let extra = opcode
            .needs_extra()
            .then_some(self.read_imem(self.regs.pc.wrapping_add(1)));

        if let Some(extra) = extra {
            ins.extra = extra;
        }

        let ins_len = opcode.len();

        // println!("executing {ins:?} at {:04X}", self.regs.pc);

        // execute
        let regs_previous = if opcode.has_extension() {
            Some(self.regs.clone())
        } else {
            None
        };

        match ins.opcode() {
            Opcode::Abs => self.abs(sys, ins),
            Opcode::Add => self.add(sys, ins),
            Opcode::Addarn => self.addarn(sys, ins),
            Opcode::Addax => self.addax(sys, ins),
            Opcode::Addaxl => self.addaxl(sys, ins),
            Opcode::Addi => self.addi(sys, ins),
            Opcode::Addis => self.addis(sys, ins),
            Opcode::Addp => self.addp(sys, ins),
            Opcode::Addpaxz => self.addpaxz(sys, ins),
            Opcode::Addr => self.addr(sys, ins),
            Opcode::Andc => self.andc(sys, ins),
            Opcode::Andcf => self.andcf(sys, ins),
            Opcode::Andf => self.andf(sys, ins),
            Opcode::Andi => self.andi(sys, ins),
            Opcode::Andr => self.andr(sys, ins),
            Opcode::Asl => self.asl(sys, ins),
            Opcode::Asr => self.asr(sys, ins),
            Opcode::Asr16 => self.asr16(sys, ins),
            Opcode::Asrn => self.asrn(sys, ins),
            Opcode::Asrnr => self.asrnr(sys, ins),
            Opcode::Asrnrx => self.asrnrx(sys, ins),
            Opcode::Bloop => self.bloop(sys, ins),
            Opcode::Bloopi => self.bloopi(sys, ins),
            Opcode::Call => self.call(sys, ins),
            Opcode::Callr => self.callr(sys, ins),
            Opcode::Clr => self.clr(sys, ins),
            Opcode::Clr15 => self.clr15(sys, ins),
            Opcode::Clrl => self.clrl(sys, ins),
            Opcode::Clrp => self.clrp(sys, ins),
            Opcode::Cmp => self.cmp(sys, ins),
            Opcode::Cmpaxh => self.cmpaxh(sys, ins),
            Opcode::Cmpi => self.cmpi(sys, ins),
            Opcode::Cmpis => self.cmpis(sys, ins),
            Opcode::Dar => self.dar(sys, ins),
            Opcode::Dec => self.dec(sys, ins),
            Opcode::Decm => self.decm(sys, ins),
            Opcode::Halt => self.halt(sys, ins),
            Opcode::Iar => self.iar(sys, ins),
            Opcode::If => self.ifcc(sys, ins),
            Opcode::Ilrr => self.ilrr(sys, ins),
            Opcode::Ilrrd => self.ilrrd(sys, ins),
            Opcode::Ilrri => self.ilrri(sys, ins),
            Opcode::Ilrrn => self.ilrrn(sys, ins),
            Opcode::Inc => self.inc(sys, ins),
            Opcode::Incm => self.incm(sys, ins),
            Opcode::Jmp => self.jmp(sys, ins),
            Opcode::Jr => self.jmpr(sys, ins),
            Opcode::Loop => self.loop_(sys, ins),
            Opcode::Loopi => self.loopi(sys, ins),
            Opcode::Lr => self.lr(sys, ins),
            Opcode::Lri => self.lri(sys, ins),
            Opcode::Lris => self.lris(sys, ins),
            Opcode::Lrr => self.lrr(sys, ins),
            Opcode::Lrrd => self.lrrd(sys, ins),
            Opcode::Lrri => self.lrri(sys, ins),
            Opcode::Lrrn => self.lrrn(sys, ins),
            Opcode::Lrs => self.lrs(sys, ins),
            Opcode::Lsl => self.lsl(sys, ins),
            Opcode::Lsl16 => self.lsl16(sys, ins),
            Opcode::Lsr => self.lsr(sys, ins),
            Opcode::Lsr16 => self.lsr16(sys, ins),
            Opcode::Lsrn => self.lsrn(sys, ins),
            Opcode::Lsrnr => self.lsrnr(sys, ins),
            Opcode::Lsrnrx => self.lsrnrx(sys, ins),
            Opcode::M0 => self.m0(sys, ins),
            Opcode::M2 => self.m2(sys, ins),
            Opcode::Madd => self.madd(sys, ins),
            Opcode::Maddc => self.maddc(sys, ins),
            Opcode::Maddx => self.maddx(sys, ins),
            Opcode::Mov => self.mov(sys, ins),
            Opcode::Movax => self.movax(sys, ins),
            Opcode::Movnp => self.movnp(sys, ins),
            Opcode::Movp => self.movp(sys, ins),
            Opcode::Movpz => self.movpz(sys, ins),
            Opcode::Movr => self.movr(sys, ins),
            Opcode::Mrr => self.mrr(sys, ins),
            Opcode::Msub => self.msub(sys, ins),
            Opcode::Msubc => self.msubc(sys, ins),
            Opcode::Msubx => self.msubx(sys, ins),
            Opcode::Mul => self.mul(sys, ins),
            Opcode::Mulac => self.mulac(sys, ins),
            Opcode::Mulaxh => self.mulaxh(sys, ins),
            Opcode::Mulc => self.mulc(sys, ins),
            Opcode::Mulcac => self.mulcac(sys, ins),
            Opcode::Mulcmv => self.mulcmv(sys, ins),
            Opcode::Mulcmvz => self.mulcmvz(sys, ins),
            Opcode::Mulmv => self.mulmv(sys, ins),
            Opcode::Mulmvz => self.mulmvz(sys, ins),
            Opcode::Mulx => self.mulx(sys, ins),
            Opcode::Mulxac => self.mulxac(sys, ins),
            Opcode::Mulxmv => self.mulxmv(sys, ins),
            Opcode::Mulxmvz => self.mulxmvz(sys, ins),
            Opcode::Neg => self.neg(sys, ins),
            Opcode::Nop | Opcode::Nx => (),
            Opcode::Not => self.not(sys, ins),
            Opcode::Orc => self.orc(sys, ins),
            Opcode::Ori => self.ori(sys, ins),
            Opcode::Orr => self.orr(sys, ins),
            Opcode::Ret => self.ret(sys, ins),
            Opcode::Rti => self.rti(sys, ins),
            Opcode::Sbclr => self.sbclr(sys, ins),
            Opcode::Sbset => self.sbset(sys, ins),
            Opcode::Set15 => self.set15(sys, ins),
            Opcode::Set16 => self.set16(sys, ins),
            Opcode::Set40 => self.set40(sys, ins),
            Opcode::Si => self.si(sys, ins),
            Opcode::Sr => self.sr(sys, ins),
            Opcode::Srr => self.srr(sys, ins),
            Opcode::Srrd => self.srrd(sys, ins),
            Opcode::Srri => self.srri(sys, ins),
            Opcode::Srrn => self.srrn(sys, ins),
            Opcode::Srs => self.srs(sys, ins),
            Opcode::Srsh => self.srsh(sys, ins),
            Opcode::Sub => self.sub(sys, ins),
            Opcode::Subarn => self.subarn(sys, ins),
            Opcode::Subax => self.subax(sys, ins),
            Opcode::Subp => self.subp(sys, ins),
            Opcode::Subr => self.subr(sys, ins),
            Opcode::Tst => self.tst(sys, ins),
            Opcode::Tstaxh => self.tstaxh(sys, ins),
            Opcode::Tstprod => self.tstprod(sys, ins),
            Opcode::Xorc => self.xorc(sys, ins),
            Opcode::Xori => self.xori(sys, ins),
            Opcode::Xorr => self.xorr(sys, ins),
            Opcode::Illegal => (),
        }

        if opcode.has_extension() {
            let regs_previous = regs_previous.unwrap();
            let extension = ins.extension_opcode();
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
                self.regs.pc += ins_len;
            } else {
                *loop_counter -= 1;
            }
        } else {
            self.regs.pc = self.regs.pc.wrapping_add(ins_len);
        }
    }
}
