use crate::Address;
use bitos::{
    BitUtils, bitos,
    integer::{u2, u4, u7, u11, u15},
};
use std::{fmt::Debug, mem::offset_of};
use strum::{FromRepr, VariantArray};

pub use powerpc;

/// Extension trait for [`Ins`](powerpc::Ins).
pub trait InsExt {
    /// GPR indicated by field rA.
    fn gpr_a(&self) -> GPR;
    /// GPR indicated by field rB.
    fn gpr_b(&self) -> GPR;
    /// GPR indicated by field rS.
    fn gpr_s(&self) -> GPR;
    /// GPR indicated by field rD.
    fn gpr_d(&self) -> GPR;
    /// SPR indicated by field SPR.
    fn spr(&self) -> SPR;
}

impl InsExt for powerpc::Ins {
    #[inline(always)]
    fn gpr_a(&self) -> GPR {
        GPR::new(self.field_ra())
    }

    #[inline(always)]
    fn gpr_b(&self) -> GPR {
        GPR::new(self.field_rb())
    }

    #[inline(always)]
    fn gpr_s(&self) -> GPR {
        GPR::new(self.field_rs())
    }

    #[inline(always)]
    fn gpr_d(&self) -> GPR {
        GPR::new(self.field_rd())
    }

    #[inline(always)]
    fn spr(&self) -> SPR {
        SPR::new(self.field_spr())
    }
}

#[bitos(4)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Cond {
    /// Whether the result has overflowed.
    #[bits(0)]
    pub ov: bool,
    /// Whether the operands are equal.
    #[bits(1)]
    pub eq: bool,
    /// Whether the first operand is greater than the second.
    #[bits(2)]
    pub gt: bool,
    /// Whether the first operand is less than the second.
    #[bits(3)]
    pub lt: bool,
}

/// The condition register (CR) contains 8 fields, named CR0-CR7, each containing flags
/// corresponding to some comparison operation.
///
/// There are two special cases:
/// - CR0: Integer instructions which have the `Rc` flag set update CR0 to contain comparisons to
///   zero and an overflow bit.
/// - CR1: Floating point instructions which have the `Rc` flag set update CR1 to contain a copy of
///   bits 0..4 of the FPSCR, indicating floating point exception status.
///
/// Other than that, comparison instructions specify one of the fields to receive the results of
/// the comparison.
#[bitos(32)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CondReg {
    // NOTE: CR0 is actually index 7! PPC bit order is big endian
    #[bits(..)]
    pub fields: [Cond; 8],
}

#[bitos(32)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MachineState {
    /// Whether little endian mode is turned on. Not supported.
    #[bits(0)]
    pub little_endian: bool,
    /// Whether the last exception is recoverable.
    #[bits(1)]
    pub recoverable_exception: bool,
    /// Whether data address translation is enabled.
    #[bits(4)]
    pub data_addr_translation: bool,
    /// Whether instruction address translation is enabled.
    #[bits(5)]
    pub instr_addr_translation: bool,
    /// Whether exception vectors are at 0x000n_nnnn (off) or 0xFFFn_nnnn (on).
    #[bits(6)]
    pub exception_prefix: bool,
    #[bits(8)]
    pub float_exception_mode_1: bool,
    #[bits(11)]
    pub float_exception_mode_0: bool,
    /// Whether machine check exceptions are enabled.
    #[bits(12)]
    pub machine_check: bool,
    /// Whether floating point instructions can be used.
    #[bits(13)]
    pub float_available: bool,
    /// Whether the processor is running in user mode.
    #[bits(14)]
    pub user_mode: bool,
    /// Whether external interrupts are enabled.
    #[bits(15)]
    pub external_interrupts: bool,
    /// Whether the CPU should be set to little endian mode after an exception occurs.
    #[bits(16)]
    pub exception_little_endian: bool,
}

/// The XER register contains information about overflow and carry operations, and is also used by
/// the load/store string indexed instructions.
#[bitos(32)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct XerReg {
    /// The number of bytes to be transferred by a lswx or stswx.
    #[bits(0..7)]
    pub byte_count: u7,
    /// Used by carrying instructions, contains the carry bit of the result.
    #[bits(29)]
    pub carry: bool,
    /// Whether an overflow has occured.
    #[bits(30)]
    pub overflow: bool,
    /// Set whenever the overflow bit is set and stays set until cleared by specific instructions.
    #[bits(31)]
    pub overflow_fuse: bool,
}

/// User level registers
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct User {
    /// General Purpose Registers
    pub gpr: [u32; 32],
    /// Floating Point Registers
    pub fpr: [f64; 32],
    /// Condition Register
    pub cr: CondReg,
    /// Floating Point Status and Condition Register
    pub fpscr: u32,

    /// XER Register
    pub xer: XerReg,
    /// Link Register
    pub lr: u32,
    /// Count Register
    pub ctr: u32,
}

/// The block address translation registers.
#[bitos(64)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bat {
    // lower
    #[bits(0..2)]
    pub protection: u2,
    #[bits(3..7)]
    pub wimg: u4,
    #[bits(17..32)]
    pub physical_address_region: u15,

    // upper
    #[bits(32)]
    pub user_mode: bool,
    #[bits(33)]
    pub supervisor_mode: bool,
    #[bits(34..45)]
    pub block_length_mask: u11,
    #[bits(49..64)]
    pub effective_address_region: u15,
}

impl Bat {
    /// The length of the memory region, in bytes.
    #[inline(always)]
    pub fn block_length(&self) -> u32 {
        (bytesize::kib(128u64) as u32) << (self.block_length_mask().value()).count_ones()
    }

    /// The start address of the memory region, inclusive.
    #[inline(always)]
    pub fn start(&self) -> Address {
        Address(
            ((self.effective_address_region().value() as u32) << 17)
            // mask the EPI with the block length! aka floor it to a multiple of block length
                & !((self.block_length_mask().value() as u32) << 17),
        )
    }

    /// The start address of the physical memory region, inclusive.
    #[inline(always)]
    pub fn physical_start(&self) -> Address {
        Address(
            ((self.physical_address_region().value() as u32) << 17)
            // mask the EPI with the block length! aka floor it to a multiple of block length
                & !((self.block_length_mask().value() as u32) << 17),
        )
    }

    /// The end address of the memory region, inclusive.
    #[inline(always)]
    pub fn end(&self) -> Address {
        self.start() + (self.block_length() - 1)
    }

    /// The end address of the memory region, inclusive.
    #[inline(always)]
    pub fn physical_end(&self) -> Address {
        self.physical_start() + (self.block_length() - 1)
    }

    /// Whether the memory region contains the given effective address.
    #[inline(always)]
    pub fn contains(&self, addr: Address) -> bool {
        (self.start()..=self.end()).contains(&addr)
    }

    /// Translates an effective address into a physical address.
    #[inline(always)]
    pub fn translate(&self, addr: Address) -> Address {
        let offset = addr.value().bits(0, 17);
        let region = ((addr.value().bits(17, 28) << 17)
            // only allow bits within the block length to be changed
            & ((self.block_length_mask().value() as u32) << 17))
            // insert the real page number
            | ((self.physical_address_region().value() as u32) << 17);

        Address(region | offset)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MemoryManagement {
    /// Instruction Block Address Translation registers
    pub ibat: [Bat; 4],
    /// Data Block Address Translation registers
    pub dbat: [Bat; 4],
    /// Segment Registers
    pub sr: [u32; 16],
    /// Page table base address (?)
    pub sdr1: u32,
}

impl MemoryManagement {
    pub fn setup_default_bats(&mut self) {
        let bat = |upper, lower| {
            use zerocopy::{
                big_endian::{U32, U64},
                transmute,
            };

            let data: U64 = transmute!([U32::new(upper), U32::new(lower)]);
            Bat::from_bits(data.get())
        };

        self.ibat[0] = bat(0x8000_1FFF, 0x0000_0002);
        self.ibat[1] = bat(0x0000_0000, 0x0000_0000);
        self.ibat[2] = bat(0x0000_0000, 0x0000_0000);
        self.ibat[3] = bat(0xFFF0_001F, 0xFFF0_0001);

        self.dbat[0] = bat(0x8000_1FFF, 0x0000_0002);
        self.dbat[1] = bat(0xC000_1FFF, 0x0000_002A);
        self.dbat[2] = bat(0x0000_0000, 0x0000_0000);
        self.dbat[3] = bat(0xFFF0_001F, 0xFFF0_0001);
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExceptionHandling {
    /// Data Address Register
    pub dar: u32,
    /// Data Storage Interrupt Status Register
    pub dsisr: u32,
    /// Registers provided for the use of the operating system
    pub sprgs: [u32; 4],
    /// Save and Restore Registers
    pub srr: [u32; 2],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Miscellaneous {
    /// Time Base
    pub tbl: u64,
    /// Decrementer
    pub dec: u32,
}

/// Supervisor level registers
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Supervisor {
    /// Machine State Register
    pub msr: MachineState,
    /// Memory management registers
    pub memory: MemoryManagement,
    /// Exception handling registers
    pub exception: ExceptionHandling,
    /// Miscellaneous registers
    pub misc: Miscellaneous,
}

impl Supervisor {
    /// Translates an instruction effective address into a physical address.
    pub fn translate_instr_addr(&self, addr: Address) -> Address {
        if !self.msr.instr_addr_translation() {
            return addr;
        }

        for bat in &self.memory.ibat {
            if bat.contains(addr) {
                return bat.translate(addr);
            }
        }

        panic!("couldn't translate instr addr {addr} with bats!")
    }

    /// Translates a data effective address into a physical address.
    pub fn translate_data_addr(&self, addr: Address) -> Address {
        if !self.msr.data_addr_translation() {
            return addr;
        }

        for bat in &self.memory.dbat {
            if bat.contains(addr) {
                return bat.translate(addr);
            }
        }

        panic!("couldn't translate data addr {addr} with bats!")
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Registers {
    /// Program Counter
    pub pc: Address,
    /// User level registers
    pub user: User,
    /// Supervisor level registers
    pub supervisor: Supervisor,
}

/// A General Purpose Register
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromRepr, VariantArray)]
#[repr(u8)]
pub enum GPR {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
    R16,
    R17,
    R18,
    R19,
    R20,
    R21,
    R22,
    R23,
    R24,
    R25,
    R26,
    R27,
    R28,
    R29,
    R30,
    R31,
}

impl GPR {
    /// Creates a new GPR with the given index.
    ///
    /// # Panics
    /// Panics if index is out of range.
    #[inline(always)]
    pub fn new(index: u8) -> Self {
        Self::from_repr(index).unwrap()
    }

    /// Offset of this GPR in the [`Registers`] struct.
    #[inline(always)]
    pub fn offset(self) -> usize {
        offset_of!(Registers, user.gpr) + size_of::<u32>() * (self as usize)
    }
}

/// A Floating Point Register
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromRepr, VariantArray)]
#[repr(u8)]
pub enum FPR {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
    R16,
    R17,
    R18,
    R19,
    R20,
    R21,
    R22,
    R23,
    R24,
    R25,
    R26,
    R27,
    R28,
    R29,
    R30,
    R31,
}

impl FPR {
    /// Creates a new FPR with the given index.
    ///
    /// # Panics
    /// Panics if index is out of range.
    #[inline(always)]
    pub fn new(index: u8) -> Self {
        Self::from_repr(index).unwrap()
    }

    /// Offset of this FPR in the [`Registers`] struct.
    #[inline(always)]
    pub fn offset(self) -> usize {
        offset_of!(Registers, user.fpr) + size_of::<f64>() * (self as usize)
    }
}

/// A Special Register
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromRepr, VariantArray)]
#[repr(u16)]
pub enum SPR {
    XER = 1,
    LR = 8,
    CTR = 9,
}

impl SPR {
    /// Creates a new SPR with the given index.
    ///
    /// # Panics
    /// Panics if index is out of range or is unknown.
    #[inline(always)]
    pub fn new(index: u16) -> Self {
        Self::from_repr(index).unwrap()
    }

    /// Offset of this SPR in the [`Registers`] struct.
    pub fn offset(self) -> usize {
        match self {
            Self::XER => offset_of!(Registers, user.xer),
            Self::LR => offset_of!(Registers, user.lr),
            Self::CTR => offset_of!(Registers, user.ctr),
        }
    }
}

/// A register in the Gekko CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reg {
    GPR(GPR),
    FPR(FPR),
    SPR(SPR),
    PC,
    MSR,
    CR,
    FPSCR,
}

impl Reg {
    /// Offset of this register in the [`Registers`] struct.
    #[inline(always)]
    pub fn offset(self) -> usize {
        match self {
            Reg::GPR(gpr) => gpr.offset(),
            Reg::FPR(fpr) => fpr.offset(),
            Reg::SPR(spr) => spr.offset(),
            Reg::PC => offset_of!(Registers, pc),
            Reg::MSR => offset_of!(Registers, supervisor.msr),
            Reg::CR => offset_of!(Registers, user.cr),
            Reg::FPSCR => offset_of!(Registers, user.fpscr),
        }
    }

    #[inline(always)]
    pub fn iter() -> impl Iterator<Item = Self> {
        let gpr = GPR::VARIANTS.iter().copied().map(Self::GPR);
        let fpr = FPR::VARIANTS.iter().copied().map(Self::FPR);
        let spr = SPR::VARIANTS.iter().copied().map(Self::SPR);
        let others = [Self::PC, Self::MSR, Self::CR, Self::FPSCR].into_iter();

        others.chain(gpr).chain(spr).chain(fpr)
    }
}

impl From<GPR> for Reg {
    #[inline(always)]
    fn from(value: GPR) -> Self {
        Self::GPR(value)
    }
}

impl From<FPR> for Reg {
    #[inline(always)]
    fn from(value: FPR) -> Self {
        Self::FPR(value)
    }
}

impl From<SPR> for Reg {
    #[inline(always)]
    fn from(value: SPR) -> Self {
        Self::SPR(value)
    }
}
