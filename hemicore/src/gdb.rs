use crate::arch::{Reg, Registers};
use binrw::{BinRead, BinWrite};
use gdbstub::arch::Arch;

impl gdbstub::arch::Registers for Registers {
    type ProgramCounter = u32;

    fn pc(&self) -> Self::ProgramCounter {
        self.pc.value()
    }

    fn gdb_serialize(&self, mut write_byte: impl FnMut(Option<u8>)) {
        let mut buf = [0; size_of::<Self>()];
        let mut cursor = std::io::Cursor::new(&mut buf[..]);
        self.write_be(&mut cursor).unwrap();

        for byte in buf {
            write_byte(Some(byte));
        }
    }

    fn gdb_deserialize(&mut self, bytes: &[u8]) -> Result<(), ()> {
        let mut buf = std::io::Cursor::new(bytes);
        *self = Self::read_be(&mut buf).map_err(|_| ())?;

        Ok(())
    }
}

impl gdbstub::arch::RegId for Reg {
    fn from_raw_id(id: usize) -> Option<(Self, Option<std::num::NonZeroUsize>)> {
        Reg::iter().nth(id).map(|reg| {
            let size = std::num::NonZeroUsize::new(if matches!(reg, Reg::FPR(_)) { 8 } else { 4 })
                .unwrap();

            (reg, Some(size))
        })
    }
}

/// Gekko arch for gdbstub.
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
