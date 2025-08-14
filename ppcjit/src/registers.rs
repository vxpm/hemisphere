use std::fmt::Debug;

use bitos::{bitos, integer::u7};

#[bitos(4)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Cond {
    /// Whether the first operand is less than the second.
    #[bits(0)]
    pub less_than: bool,
    /// Whether the first operand is greater than the second.
    #[bits(1)]
    pub greater_than: bool,
    /// Whether the operands are equal.
    #[bits(2)]
    pub equal: bool,
    /// Whether the result has overflowed.
    #[bits(3)]
    pub overflow: bool,
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
    #[bits(..)]
    pub fields: [Cond; 8],
}

/// The XER register contains information about overflow and carry operations, and is also used by
/// the load/store string indexed instructions.
#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct XerReg {
    /// Set whenever the overflow bit is set and stays set until cleared by specific instructions.
    #[bits(0)]
    pub overflow_fuse: bool,
    /// Whether an overflow has occured.
    #[bits(1)]
    pub overflow: bool,
    /// Used by carrying instructions, contains the carry bit of the result.
    #[bits(2)]
    pub carry: bool,
    /// The number of bytes to be transferred by a lswx or stswx.
    #[bits(25..32)]
    pub byte_count: u7,
}

#[repr(C)]
#[derive(Default)]
pub struct Registers {
    // == user level
    // general purpose
    pub gpr: [u32; 32],
    pub fpr: [f64; 32],
    pub cr: CondReg,
    pub fpscr: u32,

    // special purpose
    pub xer: XerReg,
    pub lr: u32,
    pub ctr: u32,
}

impl Debug for Registers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct Hex<T>(T);

        impl<T> Debug for Hex<T>
        where
            T: std::fmt::UpperHex,
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "0x{:08X}", &self.0)
            }
        }

        f.debug_struct("Registers")
            .field_with("gpr", |f| {
                let mut map = f.debug_map();
                for i in 0..32 {
                    if self.gpr[i as usize] != 0 {
                        map.entry(&i, &Hex(self.gpr[i as usize]));
                    }
                }

                map.finish_non_exhaustive()
            })
            .field_with("fpr", |f| {
                let mut map = f.debug_map();
                for i in 0..32 {
                    if self.fpr[i as usize] != 0.0 {
                        map.entry(&i, &Hex(self.fpr[i as usize] as u32));
                    }
                }

                map.finish_non_exhaustive()
            })
            .field("cr", &Hex(self.cr.to_bits()))
            .finish()
    }
}
