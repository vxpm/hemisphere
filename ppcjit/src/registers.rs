use bitos::{
    BitUtils, bitos,
    integer::{u2, u4, u7, u11, u15},
};
use bytesize::ByteSize;
use hemicore::Address;
use std::fmt::Debug;

#[bitos(4)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Cond {
    /// Whether the result has overflowed.
    #[bits(0)]
    pub overflow: bool,
    /// Whether the operands are equal.
    #[bits(1)]
    pub equal: bool,
    /// Whether the first operand is greater than the second.
    #[bits(2)]
    pub greater_than: bool,
    /// Whether the first operand is less than the second.
    #[bits(3)]
    pub less_than: bool,
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
#[derive(Debug, Clone, Copy, Default)]
pub struct CondReg {
    // NOTE: CR0 is actually index 7! PPC bit order is big endian
    #[bits(..)]
    fields: [Cond; 8],
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
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
#[derive(Debug, Clone, Copy, Default)]
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
#[derive(Debug, Default)]
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
#[derive(Debug, Default)]
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
        let region = (((addr.value().bits(17, 28) << 17)
            // only allow bits within the block length to be changed
            & ((self.block_length_mask().value() as u32) << 17)))
            // insert the real page number
            | ((self.physical_address_region().value() as u32) << 17);

        Address(region | offset)
    }
}

#[derive(Debug, Default)]
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

        for bat in &self.ibat {
            println!(
                "{} -> {}, {} -> {} ({})",
                bat.start(),
                bat.physical_start(),
                bat.end(),
                bat.physical_end(),
                ByteSize(bat.block_length() as u64),
            );
        }

        println!("");

        for bat in &self.dbat {
            println!(
                "{} -> {}, {} -> {} ({})",
                bat.start(),
                bat.physical_start(),
                bat.end(),
                bat.physical_end(),
                ByteSize(bat.block_length() as u64),
            );
        }
    }
}

#[derive(Debug, Default)]
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

#[derive(Debug, Default)]
pub struct Miscellaneous {
    /// Time Base
    pub tbl: u64,
    /// Decrementer
    pub dec: u32,
}

/// Supervisor level registers
#[repr(C)]
#[derive(Debug, Default)]
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

        panic!("couldn't translate instr addr with bats!")
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
#[derive(Debug, Default)]
pub struct Registers {
    /// User level registers
    pub user: User,
    /// Supervisor level registers
    pub supervisor: Supervisor,
}
