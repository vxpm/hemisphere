use crate::App;
use gdbstub::conn::ConnectionExt;
use gdbstub::stub::SingleThreadStopReason;
use gdbstub::stub::run_blocking::{BlockingEventLoop, Event, WaitForStopReasonError};
use gdbstub::target::Target;
use gdbstub::target::ext::base::BaseOps;
use gdbstub::target::ext::base::singlethread::SingleThreadBase;
use hemisphere::core::Address;
use hemisphere::core::gdb::Gekko;
use std::fmt::Display;
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

#[derive(Debug)]
pub struct Error;

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Target for App {
    type Arch = Gekko;
    type Error = Error;

    fn guard_rail_implicit_sw_breakpoints(&self) -> bool {
        true
    }

    fn base_ops(&mut self) -> gdbstub::target::ext::base::BaseOps<'_, Self::Arch, Self::Error> {
        BaseOps::SingleThread(self)
    }
}

impl SingleThreadBase for App {
    fn read_registers(
        &mut self,
        regs: &mut <Self::Arch as gdbstub::arch::Arch>::Registers,
    ) -> gdbstub::target::TargetResult<(), Self> {
        *regs = self.hemisphere.state.cpu.clone();
        Ok(())
    }

    fn write_registers(
        &mut self,
        regs: &<Self::Arch as gdbstub::arch::Arch>::Registers,
    ) -> gdbstub::target::TargetResult<(), Self> {
        self.hemisphere.state.cpu = regs.clone();
        Ok(())
    }

    fn read_addrs(
        &mut self,
        start_addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        data: &mut [u8],
    ) -> gdbstub::target::TargetResult<usize, Self> {
        let mut current = Address(start_addr);

        for byte in data.iter_mut() {
            let physical = self
                .hemisphere
                .state
                .cpu
                .supervisor
                .translate_instr_addr(current);

            *byte = self.hemisphere.state.bus.read(physical);
            current += 1;
        }

        Ok(data.len())
    }

    fn write_addrs(
        &mut self,
        start_addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        data: &[u8],
    ) -> gdbstub::target::TargetResult<(), Self> {
        Ok(())
    }
}

pub struct EventLoop {}

impl BlockingEventLoop for EventLoop {
    type Target = App;
    type Connection = TcpStream;
    type StopReason = SingleThreadStopReason<u32>;

    fn wait_for_stop_reason(
        target: &mut Self::Target,
        conn: &mut Self::Connection,
    ) -> Result<
        gdbstub::stub::run_blocking::Event<Self::StopReason>,
        gdbstub::stub::run_blocking::WaitForStopReasonError<
            <Self::Target as Target>::Error,
            <Self::Connection as gdbstub::conn::Connection>::Error,
        >,
    > {
        loop {
            if conn.peek().is_ok() {
                let data = conn.read().map_err(WaitForStopReasonError::Connection)?;
                break Ok(Event::IncomingData(data));
            }

            target.hemisphere.exec();
        }
    }

    fn on_interrupt(
        target: &mut Self::Target,
    ) -> Result<Option<Self::StopReason>, <Self::Target as Target>::Error> {
        todo!()
    }
}
