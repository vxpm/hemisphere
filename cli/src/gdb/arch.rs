use binrw::{BinRead, BinWrite};
use gdbstub::arch::Arch;

impl gdbstub::arch::Registers for Registers {
    type ProgramCounter = u32;

    fn pc(&self) -> Self::ProgramCounter {
        self.pc
    }

    fn gdb_serialize(&self, mut write_byte: impl FnMut(Option<u8>)) {
        let mut buf = std::io::Cursor::new(Vec::new());
        self.write_be(&mut buf).unwrap();

        for byte in buf.into_inner() {
            write_byte(Some(byte));
        }
    }

    fn gdb_deserialize(&mut self, bytes: &[u8]) -> Result<(), ()> {
        let mut buf = std::io::Cursor::new(bytes);
        *self = Self::read_be(&mut buf).map_err(|_| ())?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum Reg {
    Gpr(u8),
    Fpr(u8),
    PC,
    MSR,
    CR,
    LR,
    CTR,
    XER,
    FPSCR,
}

impl gdbstub::arch::RegId for Reg {
    fn from_raw_id(id: usize) -> Option<(Self, Option<std::num::NonZeroUsize>)> {
        use Reg::*;

        let reg = |reg, size| Some((reg, Some(std::num::NonZeroUsize::new(size).unwrap())));
        match id {
            0..32 => reg(Gpr(id as u8), 4),
            32..64 => reg(Fpr((id - 32) as u8), 8),
            64 => reg(PC, 4),
            65 => reg(MSR, 4),
            66 => reg(CR, 4),
            67 => reg(LR, 4),
            68 => reg(CTR, 4),
            69 => reg(XER, 4),
            70 => reg(FPSCR, 4),
            _ => None,
        }
    }
}

pub enum Gekko {}

impl Arch for Gekko {
    type Usize = u32;
    type Registers = Registers;
    type BreakpointKind = usize;
    type RegId = Reg;

    fn target_description_xml() -> Option<&'static str> {
        Some(
            r#"
            <target version="1.0">
                <architecture>powerpc:common</architecture>
                <feature name="org.gnu.gdb.power.core"></feature>
                <feature name="org.gnu.gdb.power.fpu"></feature>
            </target>
            "#,
        )
    }
}
