use strum::{FromRepr, VariantArray};

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
#[repr(u8)]
pub enum CondCode {
    GreaterOrEqual = 0b0000,
    Less = 0b0001,
    Greater = 0b0010,
    LessOrEqual = 0b0011,
    NotZero = 0b0100,
    Zero = 0b0101,
    NotCarry = 0b0110,
    Carry = 0b0111,
    BelowS32 = 0b1000,
    AboveS32 = 0b1001,
    WeirdA = 0b1010,
    WeirdB = 0b1011,
    NotLogicZero = 0b1100,
    LogicZero = 0b1101,
    Overflow = 0b1110,
    Always = 0b1111,
}

impl CondCode {
    pub fn new(value: u8) -> Self {
        Self::from_repr(value).unwrap()
    }
}

#[derive(Clone, Copy)]
struct OpcodeInfo {
    mask: u16,
    target: u16,
}

impl OpcodeInfo {
    #[inline(always)]
    fn matches(self, value: u16) -> bool {
        (value & self.mask) == self.target
    }

    const fn parse(s: &'static str) -> Self {
        assert!(s.is_ascii());

        let bytes = s.as_bytes();

        let mut mask = 0;
        let mut target = 0;

        let mut char_index = 0;
        let mut bit_index = 15;
        loop {
            let char = bytes[char_index];
            match char {
                b'0' => {
                    mask |= 1 << bit_index;
                }
                b'1' => {
                    mask |= 1 << bit_index;
                    target |= 1 << bit_index;
                }
                b'x' | b'_' => (),
                _ => panic!("unknown character"),
            }

            char_index += 1;
            if char != b'_' {
                if bit_index == 0 {
                    break;
                }

                bit_index -= 1;
            }
        }

        Self { mask, target }
    }
}

macro_rules! opcode {
    (
        $e:ident;
        $($name:ident = $opcode:literal),*
        $(,)?
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, VariantArray)]
        pub enum $e {
            $(
                $name,
            )*
            Illegal,
        }

        impl $e {
            pub fn new(value: u16) -> Self {
                $(
                    let info = const { OpcodeInfo::parse($opcode) };
                    if info.matches(value) {
                        return Self::$name;
                    }
                )*

                Self::Illegal
            }

            #[cfg(test)]
            fn info(self) -> Option<OpcodeInfo> {
                match self {
                    $(
                        Self::$name => Some(const { OpcodeInfo::parse($opcode) }),
                    )*
                    Self::Illegal => None,
                }
            }
        }
    };
}

opcode! {
    Opcode;
    Nop     = "0000_0000_0000_0000",
    Dar     = "0000_0000_0000_01xx",
    Iar     = "0000_0000_0000_10xx",
    Subarn  = "0000_0000_0000_11xx",
    Addarn  = "0000_0000_0001_xxxx",
    Halt    = "0000_0000_0010_0001",
    Loop    = "0000_0000_010x_xxxx",
    Bloop   = "0000_0000_011x_xxxx",

    Lri     = "0000_0000_100x_xxxx",
    Lr      = "0000_0000_110x_xxxx",
    Sr      = "0000_0000_111x_xxxx",

    If      = "0000_0010_0111_xxxx",
    Jmp     = "0000_0010_1001_xxxx",
    Call    = "0000_0010_1011_xxxx",
    Ret     = "0000_0010_1101_xxxx",
    Rti     = "0000_0010_1111_xxxx",

    Addi    = "0000_001x_0000_0000",
    Xori    = "0000_001x_0010_0000",
    Andi    = "0000_001x_0100_0000",
    Ori     = "0000_001x_0110_0000",
    Cmpi    = "0000_001x_1000_0000",
    Andf    = "0000_001x_1010_0000",
    Andcf   = "0000_001x_1100_0000",
    Lsrn    = "0000_0010_1100_1010",
    Asrn    = "0000_0010_1100_1011",

    Ilrr    = "0000_001x_0001_00xx",
    Ilrrd   = "0000_001x_0001_01xx",
    Ilrri   = "0000_001x_0001_10xx",
    Ilrrn   = "0000_001x_0001_11xx",

    Addis   = "0000_010x_xxxx_xxxx",
    Cmpis   = "0000_011x_xxxx_xxxx",
    Lris    = "0000_1xxx_xxxx_xxxx",

    Loopi   = "0001_0000_xxxx_xxxx",
    Bloopi  = "0001_0001_xxxx_xxxx",
    Sbclr   = "0001_0010_xxxx_xxxx",
    Sbset   = "0001_0011_xxxx_xxxx",

    Lsl     = "0001_010x_00xx_xxxx",
    Lsr     = "0001_010x_01xx_xxxx",
    Asl     = "0001_010x_10xx_xxxx",
    Asr     = "0001_010x_11xx_xxxx",
    Si      = "0001_0110_xxxx_xxxx",
    Jr      = "0001_0111_xxx0_xxxx",
    Callr   = "0001_0111_xxx1_xxxx",

    Lrr     = "0001_1000_0xxx_xxxx",
    Lrrd    = "0001_1000_1xxx_xxxx",
    Lrri    = "0001_1001_0xxx_xxxx",
    Lrrn    = "0001_1001_1xxx_xxxx",
    Srr     = "0001_1010_0xxx_xxxx",
    Srrd    = "0001_1010_1xxx_xxxx",
    Srri    = "0001_1011_0xxx_xxxx",
    Srrn    = "0001_1011_1xxx_xxxx",
    Mrr     = "0001_11xx_xxxx_xxxx",

    Lrs     = "0010_0xxx_xxxx_xxxx",
    Srsh    = "0010_100x_xxxx_xxxx",
    Srs     = "0010_11xx_xxxx_xxxx",

    Xorr    = "0011_00xx_0xxx_xxxx",
    Andr    = "0011_01xx_0xxx_xxxx",
    Orr     = "0011_10xx_0xxx_xxxx",
    Andc    = "0011_110x_0xxx_xxxx",
    Orc     = "0011_111x_0xxx_xxxx",

    Xorc    = "0011_000x_1xxx_xxxx",
    Not     = "0011_001x_1xxx_xxxx",
    Lsrnrx  = "0011_01xx_1xxx_xxxx",
    Asrnrx  = "0011_10xx_1xxx_xxxx",
    Lsrnr   = "0011_110x_1xxx_xxxx",
    Asrnr   = "0011_111x_1xxx_xxxx",

    Addr    = "0100_0xxx_xxxx_xxxx",
    Addax   = "0100_10xx_xxxx_xxxx",
    Add     = "0100_110x_xxxx_xxxx",
    Addp    = "0100_111x_xxxx_xxxx",

    Subr    = "0101_0xxx_xxxx_xxxx",
    Subax   = "0101_10xx_xxxx_xxxx",
    Sub     = "0101_110x_xxxx_xxxx",
    Subp    = "0101_111x_xxxx_xxxx",

    Movr    = "0110_0xxx_xxxx_xxxx",
    Movax   = "0110_10xx_xxxx_xxxx",
    Mov     = "0110_110x_xxxx_xxxx",
    Movp    = "0110_111x_xxxx_xxxx",

    Addaxl  = "0111_00xx_xxxx_xxxx",
    Incm    = "0111_010x_xxxx_xxxx",
    Inc     = "0111_011x_xxxx_xxxx",
    Decm    = "0111_100x_xxxx_xxxx",
    Dec     = "0111_101x_xxxx_xxxx",
    Neg     = "0111_110x_xxxx_xxxx",
    Movnp   = "0111_111x_xxxx_xxxx",

    Nx      = "1000_x000_xxxx_xxxx",
    Clr     = "1000_x001_xxxx_xxxx",
    Cmp     = "1000_0010_xxxx_xxxx",
    Mulaxh  = "1000_0011_xxxx_xxxx",
    Clrp    = "1000_0100_xxxx_xxxx",
    Tstprod = "1000_0101_xxxx_xxxx",
    Tstaxh  = "1000_011x_xxxx_xxxx",

    M2      = "1000_1010_xxxx_xxxx",
    M0      = "1000_1011_xxxx_xxxx",
    Clr15   = "1000_1100_xxxx_xxxx",
    Set15   = "1000_1101_xxxx_xxxx",
    Set16   = "1000_1110_xxxx_xxxx",
    Set40   = "1000_1111_xxxx_xxxx",

    Mul     = "1001_x000_xxxx_xxxx",
    Asr16   = "1001_x001_xxxx_xxxx",
    Mulmvz  = "1001_x01x_xxxx_xxxx",
    Mulac   = "1001_x10x_xxxx_xxxx",
    Mulmv   = "1001_x11x_xxxx_xxxx",

    Mulx    = "101x_x000_xxxx_xxxx",
    Abs     = "1010_x001_xxxx_xxxx",
    Tst     = "1011_x001_xxxx_xxxx",
    Mulxmvz = "101x_x01x_xxxx_xxxx",
    Mulxac  = "101x_x10x_xxxx_xxxx",
    Mulxmv  = "101x_x11x_xxxx_xxxx",

    Mulc    = "110x_x000_xxxx_xxxx",
    Cmpaxh  = "110x_x001_xxxx_xxxx",
    Mulcmvz = "110x_x01x_xxxx_xxxx",
    Mulcac  = "110x_x10x_xxxx_xxxx",
    Mulcmv  = "110x_x11x_xxxx_xxxx",

    Maddx   = "1110_00xx_xxxx_xxxx",
    Msubx   = "1110_01xx_xxxx_xxxx",
    Maddc   = "1110_10xx_xxxx_xxxx",
    Msubc   = "1110_11xx_xxxx_xxxx",

    Lsl16   = "1111_000x_xxxx_xxxx",
    Madd    = "1111_001x_xxxx_xxxx",
    Lsr16   = "1111_010x_xxxx_xxxx",
    Msub    = "1111_011x_xxxx_xxxx",
    Addpaxz = "1111_10xx_xxxx_xxxx",
    Clrl    = "1111_110x_xxxx_xxxx",
    Movpz   = "1111_111x_xxxx_xxxx",
}

impl Opcode {
    pub fn needs_extra(self) -> bool {
        use Opcode::*;

        matches!(
            self,
            Bloop
                | Lri
                | Lr
                | Sr
                | Jmp
                | Call
                | Addi
                | Xori
                | Andi
                | Ori
                | Cmpi
                | Andf
                | Andcf
                | Bloopi
                | Si
        )
    }

    pub fn has_extension(self) -> bool {
        use Opcode::*;

        matches!(
            self,
            Xorr | Andr
                | Orr
                | Andc
                | Orc
                | Xorc
                | Not
                | Lsrnrx
                | Asrnrx
                | Lsrnr
                | Asrnr
                | Addr
                | Addax
                | Add
                | Addp
                | Subr
                | Subax
                | Sub
                | Subp
                | Movr
                | Movax
                | Mov
                | Movp
                | Addaxl
                | Incm
                | Inc
                | Decm
                | Dec
                | Neg
                | Movnp
                | Nx
                | Clr
                | Cmp
                | Mulaxh
                | Clrp
                | Tstprod
                | Tstaxh
                | M2
                | M0
                | Clr15
                | Set15
                | Set16
                | Set40
                | Mul
                | Asr16
                | Mulmvz
                | Mulac
                | Mulmv
                | Mulx
                | Abs
                | Tst
                | Mulxmvz
                | Mulxac
                | Mulxmv
                | Mulc
                | Cmpaxh
                | Mulcmvz
                | Mulcac
                | Mulcmv
                | Maddx
                | Msubx
                | Maddc
                | Msubc
                | Lsl16
                | Madd
                | Lsr16
                | Msub
                | Addpaxz
                | Clrl
                | Movpz
        )
    }

    pub fn extension_mask(&self) -> u16 {
        use Opcode::*;

        match self {
            Asrnr => 0x7F,
            Asrnrx => 0x7F,
            _ => 0xFF,
        }
    }
}

opcode! {
    ExtensionOpcode;
    Nop     = "xxxx_xxxx_0000_00xx",
    Dr      = "xxxx_xxxx_0000_01xx",
    Ir      = "xxxx_xxxx_0000_10xx",
    Nr      = "xxxx_xxxx_0000_11xx",
    Mv      = "xxxx_xxxx_0001_xxxx",
    S       = "xxxx_xxxx_001x_x0xx",
    Sn      = "xxxx_xxxx_001x_x1xx",
    L       = "xxxx_xxxx_01xx_x0xx",
    Ln      = "xxxx_xxxx_01xx_x1xx",

    Ls      = "xxxx_xxxx_10xx_000x",
    Sl      = "xxxx_xxxx_10xx_001x",
    Lsn     = "xxxx_xxxx_10xx_010x",
    Sln     = "xxxx_xxxx_10xx_011x",
    Lsm     = "xxxx_xxxx_10xx_100x",
    Slm     = "xxxx_xxxx_10xx_101x",
    Lsnm    = "xxxx_xxxx_10xx_110x",
    Slnm    = "xxxx_xxxx_10xx_111x",

    Ld      = "xxxx_xxxx_11xx_00xx",
    Ldn     = "xxxx_xxxx_11xx_01xx",
    Ldm     = "xxxx_xxxx_11xx_10xx",
    Ldnm    = "xxxx_xxxx_11xx_11xx",
}

#[derive(Debug, Clone, Copy)]
pub struct Ins {
    pub base: u16,
    pub extra: u16,
}

impl Ins {
    pub fn new(base: u16) -> Self {
        Self { base, extra: 0 }
    }

    pub fn with_extra(base: u16, extra: u16) -> Self {
        Self { base, extra }
    }

    pub fn opcode(self) -> Opcode {
        Opcode::new(self.base)
    }

    pub fn extension_opcode(self) -> ExtensionOpcode {
        let opcode = self.opcode();
        let mask = opcode.extension_mask();

        ExtensionOpcode::new(self.base & mask)
    }
}

#[cfg(test)]
mod test {
    use super::{ExtensionOpcode, Opcode};
    use strum::VariantArray;

    #[test]
    fn unique_opcodes() {
        for value in 0..u16::MAX {
            let mut hit = None;
            for opcode in Opcode::VARIANTS {
                if opcode.info().is_some_and(|i| i.matches(value)) {
                    if let Some(hit) = hit {
                        panic!("opcodes {hit:?} and {opcode:?} are valid for {value:016b}");
                    }

                    hit = Some(*opcode);
                }
            }
        }
    }

    #[test]
    fn unique_extension_opcodes() {
        for value in 0..u16::MAX {
            let mut hit = None;
            for opcode in ExtensionOpcode::VARIANTS {
                if opcode.info().is_some_and(|i| i.matches(value)) {
                    if let Some(hit) = hit {
                        panic!(
                            "extension opcodes {hit:?} and {opcode:?} are valid for {value:016b}"
                        );
                    }

                    hit = Some(*opcode);
                }
            }
        }
    }
}
