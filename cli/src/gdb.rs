use crate::Emulator;
use gdbstub::target::Target;
use gdbstub::target::ext::base::BaseOps;
use gdbstub::target::ext::base::singlethread::SingleThreadBase;
use hemisphere::core::gdb::Gekko;
use std::io;
use std::net::{TcpListener, TcpStream};
use tracing::info;

pub fn connect(port: u16) -> io::Result<TcpStream> {
    let listen_addr = format!("localhost:{}", port);

    info!("Waiting for a GDB connection on {:?}...", listen_addr);
    let sock = TcpListener::bind(listen_addr)?;
    let (stream, addr) = sock.accept()?;
    info!("Debugger connected from {}", addr);

    Ok(stream)
}

impl Target for Emulator {
    type Arch = Gekko;
    type Error = ();

    fn base_ops(&mut self) -> gdbstub::target::ext::base::BaseOps<'_, Self::Arch, Self::Error> {
        BaseOps::SingleThread(self)
    }
}

impl SingleThreadBase for Emulator {
    fn read_registers(
        &mut self,
        regs: &mut <Self::Arch as gdbstub::arch::Arch>::Registers,
    ) -> gdbstub::target::TargetResult<(), Self> {
        *regs = self.hemisphere.cpu.clone();
        Ok(())
    }

    fn write_registers(
        &mut self,
        regs: &<Self::Arch as gdbstub::arch::Arch>::Registers,
    ) -> gdbstub::target::TargetResult<(), Self> {
        self.hemisphere.cpu = regs.clone();
        Ok(())
    }

    fn read_addrs(
        &mut self,
        start_addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        data: &mut [u8],
    ) -> gdbstub::target::TargetResult<usize, Self> {
        Ok(0)
    }

    fn write_addrs(
        &mut self,
        start_addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        data: &[u8],
    ) -> gdbstub::target::TargetResult<(), Self> {
        Ok(())
    }
}
