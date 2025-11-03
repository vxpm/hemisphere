mod exec;

pub mod ins;
pub mod mmio;

use crate::{
    ins::{ExtensionOpcode, Opcode},
    mmio::Mmio,
};
use bitos::{BitUtils, bitos, integer::u15};
use common::util::boxed_array;
use strum::FromRepr;
use tinyvec::ArrayVec;

pub use ins::Ins;

const ARAM_LEN: usize = 16 * bytesize::MIB as usize;
const IRAM_LEN: usize = 0x1000;
const IROM_LEN: usize = 0x1000;
const DRAM_LEN: usize = 0x1000;
const COEF_LEN: usize = 0x0800;

pub struct Memory {
    pub aram: Box<[u8; ARAM_LEN]>,
    pub iram: Box<[u16; IRAM_LEN]>,
    pub irom: Box<[u16; IROM_LEN]>,
    pub dram: Box<[u16; DRAM_LEN]>,
    pub coef: Box<[u16; COEF_LEN]>,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            aram: boxed_array(0),
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
pub struct Dsp {
    pub regs: Registers,
    pub mem: Memory,
    pub mmio: Mmio,
    pub loop_counter: Option<u16>,
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
                let addr = *self.regs.call_stack.last().unwrap();
                self.regs.pc = addr;
            }
        }
    }

    /// Soft resets the DSP.
    pub fn reset(&mut self) {
        self.loop_counter = None;

        self.regs.wrapping = [0xFFFF; 4];
        self.mmio.dsp_mailbox = mmio::Mailbox::from_bits(0);
        self.mmio.cpu_mailbox = mmio::Mailbox::from_bits(0);

        // self.regs.call_stack.push(self.regs.pc);
        // self.regs.data_stack.push(self.regs.status.to_bits());

        self.regs.pc = if self.mmio.control.reset_high() {
            0x8000
        } else {
            0x0000
        };
    }

    pub fn read_mmio(&mut self, offset: u8) -> u16 {
        match offset {
            // DMA
            0xC9 => self.mmio.dsp_dma.control.to_bits(),
            0xCB => self.mmio.dsp_dma.length,
            0xCD => self.mmio.dsp_dma.dsp_base,
            0xCE => (self.mmio.dsp_dma.ram_base >> 16) as u16,
            0xCF => self.mmio.dsp_dma.ram_base as u16,

            // Mailboxes
            0xFC => self.mmio.dsp_mailbox.high_and_status(),
            0xFD => self.mmio.dsp_mailbox.low(),
            0xFE => self.mmio.cpu_mailbox.high_and_status(),
            0xFF => {
                self.mmio.cpu_mailbox.set_status(false);
                self.mmio.cpu_mailbox.low()
            }
            _ => unimplemented!("read from {offset:02X}"),
        }
    }

    pub fn write_mmio(&mut self, offset: u8, value: u16) {
        match offset {
            // Coefficients
            0xA0..=0xAF => (),

            // DMA
            0xC9 => {
                self.mmio.dsp_dma.control = mmio::DspDmaControl::from_bits(value)
                    .with_transfer_ongoing(self.mmio.dsp_dma.control.transfer_ongoing())
            }
            0xCB => self.mmio.dsp_dma.length = value, // TODO: this
            0xCD => self.mmio.dsp_dma.dsp_base = value,
            0xCE => {
                self.mmio.dsp_dma.ram_base =
                    self.mmio.dsp_dma.ram_base.with_bits(16, 32, value as u32)
            }
            0xCF => {
                self.mmio.dsp_dma.ram_base =
                    self.mmio.dsp_dma.ram_base.with_bits(0, 16, value as u32)
            }

            // ARAM
            0xD1..=0xED => (),

            // Interrupt
            0xFB => {
                self.mmio.control.set_dsp_interrupt(true);
            }

            // Mailboxes
            0xFC => {
                self.mmio.dsp_mailbox.set_high(u15::new(value));
            }
            0xFD => {
                self.mmio.dsp_mailbox.set_low(value);
                self.mmio.dsp_mailbox.set_status(true);
            }
            _ => unimplemented!("write to {offset:02X}"),
        }
    }

    /// Reads from data memory.
    pub fn read_dmem(&mut self, addr: u16) -> u16 {
        println!("read dmem 0x{addr:04X}");
        let value = match addr {
            0x0000..0x1000 => self.mem.dram[addr as usize],
            0x1000..0x1800 => self.mem.coef[addr as usize - 0x1000],
            0xFF00.. => self.read_mmio(addr as u8),
            _ => 0,
        };

        value
    }

    /// Writes to data memory.
    pub fn write_dmem(&mut self, addr: u16, value: u16) {
        match addr {
            0x0000..0x1000 => self.mem.dram[addr as usize] = value,
            0x1000..0x1800 => self.mem.coef[addr as usize - 0x1000] = value,
            0xFF00.. => self.write_mmio(addr as u8, value),
            _ => (),
        }
    }

    /// Reads from instruction memory.
    pub fn read_imem(&mut self, addr: u16) -> u16 {
        match addr {
            0x0000..0x1000 => self.mem.iram[addr as usize],
            0x8000..0x9000 => self.mem.irom[addr as usize - 0x8000],
            _ => 0,
        }
    }

    /// Writes to instruction memory.
    pub fn write_imem(&mut self, addr: u16, value: u16) {
        match addr {
            0x0000..0x1000 => self.mem.iram[addr as usize] = value,
            _ => (),
        }
    }

    pub fn step(&mut self) {
        self.check_stacks();

        // fetch
        let mut ins = Ins::new(self.read_imem(self.regs.pc));
        let opcode = ins.opcode();

        let extra = opcode
            .needs_extra()
            .then_some(self.read_imem(self.regs.pc.wrapping_add(1)));

        if let Some(extra) = extra {
            ins.extra = extra;
        }

        if opcode != Opcode::Nop && opcode != Opcode::Lrri {
            println!("executing {:?} at {:04X}", ins, self.regs.pc);
        }

        // execute
        let regs_previous = self.regs.clone();
        match ins.opcode() {
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
            Opcode::Asr16 => self.asr16(ins),
            Opcode::Asrn => self.asrn(ins),
            Opcode::Asrnr => self.asrnr(ins),
            Opcode::Asrnrx => self.asrnrx(ins),
            Opcode::Bloop => self.bloop(ins),
            Opcode::Bloopi => self.bloopi(ins),
            Opcode::Call => self.call(ins),
            Opcode::Callr => self.callr(ins),
            Opcode::Clr => self.clr(ins),
            Opcode::Clr15 => self.clr15(ins),
            Opcode::Clrl => self.clrl(ins),
            Opcode::Clrp => self.clrp(ins),
            Opcode::Cmp => self.cmp(ins),
            Opcode::Cmpaxh => self.cmpaxh(ins),
            Opcode::Cmpi => self.cmpi(ins),
            Opcode::Cmpis => self.cmpis(ins),
            Opcode::Dar => self.dar(ins),
            Opcode::Dec => self.dec(ins),
            Opcode::Decm => self.decm(ins),
            Opcode::Halt => self.halt(ins),
            Opcode::Iar => self.iar(ins),
            Opcode::If => self.ifcc(ins),
            Opcode::Ilrr => self.ilrr(ins),
            Opcode::Ilrrd => self.ilrrd(ins),
            Opcode::Ilrri => self.ilrri(ins),
            Opcode::Ilrrn => self.ilrrn(ins),
            Opcode::Inc => self.inc(ins),
            Opcode::Incm => self.incm(ins),
            Opcode::Jmp => self.jmp(ins),
            Opcode::Jr => self.jmpr(ins),
            Opcode::Loop => self.loop_(ins),
            Opcode::Loopi => self.loopi(ins),
            Opcode::Lr => self.lr(ins),
            Opcode::Lri => self.lri(ins),
            Opcode::Lris => self.lris(ins),
            Opcode::Lrr => self.lrr(ins),
            Opcode::Lrrd => self.lrrd(ins),
            Opcode::Lrri => self.lrri(ins),
            Opcode::Lrrn => self.lrrn(ins),
            Opcode::Lrs => self.lrs(ins),
            Opcode::Lsl => self.lsl(ins),
            Opcode::Lsl16 => self.lsl16(ins),
            Opcode::Lsr => self.lsr(ins),
            Opcode::Lsr16 => self.lsr16(ins),
            Opcode::Lsrn => self.lsrn(ins),
            Opcode::Lsrnr => self.lsrnr(ins),
            Opcode::Lsrnrx => self.lsrnrx(ins),
            Opcode::M0 => self.m0(ins),
            Opcode::M2 => self.m2(ins),
            Opcode::Madd => self.madd(ins),
            Opcode::Maddc => self.maddc(ins),
            Opcode::Maddx => self.maddx(ins),
            Opcode::Mov => self.mov(ins),
            Opcode::Movax => self.movax(ins),
            Opcode::Movnp => self.movnp(ins),
            Opcode::Movp => self.movp(ins),
            Opcode::Movpz => self.movpz(ins),
            Opcode::Movr => self.movr(ins),
            Opcode::Mrr => self.mrr(ins),
            Opcode::Msub => self.msub(ins),
            Opcode::Msubc => self.msubc(ins),
            Opcode::Msubx => self.msubx(ins),
            Opcode::Mul => self.mul(ins),
            Opcode::Mulac => self.mulac(ins),
            Opcode::Mulaxh => self.mulaxh(ins),
            Opcode::Mulc => self.mulc(ins),
            Opcode::Mulcac => self.mulcac(ins),
            Opcode::Mulcmv => self.mulcmv(ins),
            Opcode::Mulcmvz => self.mulcmvz(ins),
            Opcode::Mulmv => self.mulmv(ins),
            Opcode::Mulmvz => self.mulmvz(ins),
            Opcode::Mulx => self.mulx(ins),
            Opcode::Mulxac => self.mulxac(ins),
            Opcode::Mulxmv => self.mulxmv(ins),
            Opcode::Mulxmvz => self.mulxmvz(ins),
            Opcode::Neg => self.neg(ins),
            Opcode::Nop | Opcode::Nx => (),
            Opcode::Not => self.not(ins),
            Opcode::Orc => self.orc(ins),
            Opcode::Ori => self.ori(ins),
            Opcode::Orr => self.orr(ins),
            Opcode::Ret => self.ret(ins),
            Opcode::Rti => self.rti(ins),
            Opcode::Sbclr => self.sbclr(ins),
            Opcode::Sbset => self.sbset(ins),
            Opcode::Set15 => self.set15(ins),
            Opcode::Set16 => self.set16(ins),
            Opcode::Set40 => self.set40(ins),
            Opcode::Si => self.si(ins),
            Opcode::Sr => self.sr(ins),
            Opcode::Srr => self.srr(ins),
            Opcode::Srrd => self.srrd(ins),
            Opcode::Srri => self.srri(ins),
            Opcode::Srrn => self.srrn(ins),
            Opcode::Srs => self.srs(ins),
            Opcode::Srsh => self.srsh(ins),
            Opcode::Sub => self.sub(ins),
            Opcode::Subarn => self.subarn(ins),
            Opcode::Subax => self.subax(ins),
            Opcode::Subp => self.subp(ins),
            Opcode::Subr => self.subr(ins),
            Opcode::Tst => self.tst(ins),
            Opcode::Tstaxh => self.tstaxh(ins),
            Opcode::Tstprod => self.tstprod(ins),
            Opcode::Xorc => self.xorc(ins),
            Opcode::Xori => self.xori(ins),
            Opcode::Xorr => self.xorr(ins),
            Opcode::Illegal => (),
        }

        if opcode.has_extension() {
            let extension = ins.extension_opcode();
            match extension {
                ExtensionOpcode::Dr => self.ext_dr(ins, &regs_previous),
                ExtensionOpcode::Ir => self.ext_ir(ins, &regs_previous),
                ExtensionOpcode::L => self.ext_l(ins, &regs_previous),
                ExtensionOpcode::Ld => self.ext_ld(ins, &regs_previous),
                ExtensionOpcode::Ldm => self.ext_ldm(ins, &regs_previous),
                ExtensionOpcode::Ldn => self.ext_ldn(ins, &regs_previous),
                ExtensionOpcode::Ldnm => self.ext_ldnm(ins, &regs_previous),
                ExtensionOpcode::Ln => self.ext_ln(ins, &regs_previous),
                ExtensionOpcode::Ls => self.ext_ls(ins, &regs_previous),
                ExtensionOpcode::Lsm => self.ext_lsm(ins, &regs_previous),
                ExtensionOpcode::Lsn => self.ext_lsn(ins, &regs_previous),
                ExtensionOpcode::Lsnm => self.ext_lsnm(ins, &regs_previous),
                ExtensionOpcode::Mv => self.ext_mv(ins, &regs_previous),
                ExtensionOpcode::Nop => (),
                ExtensionOpcode::Nr => self.ext_nr(ins, &regs_previous),
                ExtensionOpcode::S => self.ext_s(ins, &regs_previous),
                ExtensionOpcode::Sl => self.ext_sl(ins, &regs_previous),
                ExtensionOpcode::Slm => self.ext_slm(ins, &regs_previous),
                ExtensionOpcode::Sln => self.ext_sln(ins, &regs_previous),
                ExtensionOpcode::Slnm => self.ext_slnm(ins, &regs_previous),
                ExtensionOpcode::Sn => self.ext_sn(ins, &regs_previous),
                ExtensionOpcode::Illegal => panic!("illegal extension opcode"),
            }
        }

        if let Some(loop_counter) = &mut self.loop_counter {
            if *loop_counter == 0 {
                self.loop_counter = None;
                self.regs.pc += 1;
            } else {
                *loop_counter -= 1;
            }
        } else {
            self.regs.pc = self
                .regs
                .pc
                .wrapping_add(if extra.is_some() { 2 } else { 1 });
        }
    }
}
