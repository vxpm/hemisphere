use std::fmt::Debug;

use bitos::{bitos, integer::u7};

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

#[derive(Debug, Default)]
pub struct MemoryManagement {
    /// Instruction Block Address Translation registers
    pub ibat: [u32; 8],
    /// Data Block Address Translation registers
    pub dbat: [u32; 8],
    /// Segment Registers
    pub sr: [u32; 16],
    /// Page table base address (?)
    pub sdr1: u32,
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
    pub msr: u32,
    /// Memory management registers
    pub memory: MemoryManagement,
    /// Exception handling registers
    pub exception: ExceptionHandling,
    /// Miscellaneous registers
    pub misc: Miscellaneous,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Registers {
    /// User level registers
    pub user: User,
    /// Supervisor level registers
    pub supervisor: Supervisor,
}

// impl Debug for Registers {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         struct Hex<T>(T);
//
//         impl<T> Debug for Hex<T>
//         where
//             T: std::fmt::UpperHex,
//         {
//             fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//                 write!(f, "0x{:08X}", &self.0)
//             }
//         }
//
//         f.debug_struct("Registers")
//             .field_with("gpr", |f| {
//                 let mut map = f.debug_map();
//                 for i in 0..32 {
//                     if self.gpr[i as usize] != 0 {
//                         map.entry(&i, &Hex(self.gpr[i as usize]));
//                     }
//                 }
//
//                 map.finish_non_exhaustive()
//             })
//             .field_with("fpr", |f| {
//                 let mut map = f.debug_map();
//                 for i in 0..32 {
//                     if self.fpr[i as usize] != 0.0 {
//                         map.entry(&i, &Hex(self.fpr[i as usize] as u32));
//                     }
//                 }
//
//                 map.finish_non_exhaustive()
//             })
//             .field("cr", &Hex(self.cr.to_bits()))
//             .finish()
//     }
// }
