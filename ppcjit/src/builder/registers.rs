use crate::Registers;
use cranelift::codegen::ir;
use num_enum::TryFromPrimitive;
use std::mem::offset_of;

#[derive(Clone, Copy, PartialEq, Eq, Hash, TryFromPrimitive)]
#[repr(u16)]
pub enum Spr {
    XER = 1,
    LR = 8,
    CTR = 9,
}

impl Spr {
    pub fn offset(&self) -> i32 {
        let offset = match self {
            Self::XER => offset_of!(Registers, user.xer),
            Self::LR => offset_of!(Registers, user.lr),
            Self::CTR => offset_of!(Registers, user.ctr),
        };

        offset as i32
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[expect(dead_code, reason = "still not used")]
pub enum Reg {
    Gpr(u8),
    Fpr(u8),
    Spr(Spr),
    Cr,
}

impl Reg {
    #[inline]
    pub fn ty(self) -> ir::Type {
        match self {
            Reg::Fpr(_) => ir::types::F64,
            _ => ir::types::I32,
        }
    }

    #[inline]
    pub fn offset(self) -> i32 {
        let offset = match self {
            Reg::Gpr(i) => {
                assert!(i < 32);
                offset_of!(Registers, user.gpr) + size_of::<u32>() * (i as usize)
            }
            Reg::Fpr(i) => {
                assert!(i < 32);
                offset_of!(Registers, user.fpr) + size_of::<f64>() * (i as usize)
            }
            Reg::Cr => offset_of!(Registers, user.cr),
            Reg::Spr(spr) => return spr.offset(),
        };

        offset as i32
    }
}
