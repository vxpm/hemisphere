use crate::arch::{FPR, GPR, Reg, Registers};
use gdbstub::arch::Arch;

impl gdbstub::arch::RegId for Reg {
    fn from_raw_id(id: usize) -> Option<(Self, Option<std::num::NonZeroUsize>)> {
        fn reg(r: impl Into<Reg>, size: usize) -> (Reg, Option<std::num::NonZeroUsize>) {
            let size = std::num::NonZeroUsize::new(size).unwrap();
            (r.into(), Some(size))
        }

        Some(match id {
            0..32 => reg(GPR::new(id as u8), 4),
            32..64 => reg(FPR::new((id - 32) as u8), 4),
            64 => reg(Reg::PC, 4),
            65 => reg(Reg::MSR, 4),
            66 => reg(Reg::CR, 4),
            67 => reg(Reg::LR, 4),
            68 => reg(Reg::CTR, 4),
            69 => reg(Reg::XER, 4),
            70 => reg(Reg::FPSCR, 4),
            _ => return None,
        })
    }
}

impl gdbstub::arch::Registers for Registers {
    type ProgramCounter = u32;

    fn pc(&self) -> Self::ProgramCounter {
        self.pc.value()
    }

    fn gdb_serialize(&self, mut write_byte: impl FnMut(Option<u8>)) {
        let mut write = |bytes: &[u8]| {
            for &byte in bytes {
                write_byte(Some(byte))
            }
        };

        for gpr in self.user.gpr {
            write(&gpr.to_be_bytes());
        }

        for fpr in self.user.fpr {
            write(&fpr.to_be_bytes());
        }

        write(&self.pc.value().to_be_bytes());
        write(&self.supervisor.msr.to_bits().to_be_bytes());
        write(&self.user.cr.to_bits().to_be_bytes());
        write(&self.user.lr.to_be_bytes());
        write(&self.user.ctr.to_be_bytes());
        write(&self.user.xer.to_bits().to_be_bytes());
        write(&self.user.fpscr.to_be_bytes());
    }

    fn gdb_deserialize(&mut self, bytes: &[u8]) -> Result<(), ()> {
        // let mut buf = std::io::Cursor::new(bytes);
        // *self = Self::read_be(&mut buf).map_err(|_| ())?;

        Ok(())
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
