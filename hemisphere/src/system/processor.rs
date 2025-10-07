//! Processor interface.

use crate::system::{Event, System};
use bitos::{bitos, integer::u14};
use common::{Address, arch::Exception};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interrupt {
    GpError,
    Reset,
    DVD,
    Serial,
    External,
    Audio,
    DSP,
    Memory,
    Video,
    PeToken,
    PeFinish,
    CommandProcessor,
    Debug,
    HighSpeedPort,
}

#[bitos(14)]
#[derive(Default, Debug, Clone, Copy)]
pub struct Sources {
    #[bits(0)]
    pub gp_error: bool,
    #[bits(1)]
    pub reset: bool,
    #[bits(2)]
    pub dvd_interface: bool,
    #[bits(3)]
    pub serial_interface: bool,
    #[bits(4)]
    pub external_interface: bool,
    #[bits(5)]
    pub audio_interface: bool,
    #[bits(6)]
    pub dsp_interface: bool,
    #[bits(7)]
    pub memory_interface: bool,
    #[bits(8)]
    pub video_interface: bool,
    #[bits(9)]
    pub pe_token: bool,
    #[bits(10)]
    pub pe_finish: bool,
    #[bits(11)]
    pub command_processor: bool,
    #[bits(12)]
    pub debug: bool,
    #[bits(13)]
    pub high_speed_port: bool,
}

#[bitos(32)]
#[derive(Default, Debug, Clone, Copy)]
pub struct InterruptMask {
    #[bits(0..14)]
    pub sources: Sources,
}

#[bitos(32)]
#[derive(Default, Debug, Clone, Copy)]
pub struct InterruptCause {
    #[bits(0..14)]
    pub sources: Sources,
    #[bits(16)]
    pub reset_state: bool,
}

#[derive(Default)]
pub struct Interface {
    // interrupts
    pub mask: InterruptMask,
    pub cause: InterruptCause,

    // fifo
    pub fifo_start: Address,
    pub fifo_end: Address,
    pub fifo_current: Address,
    pub fifo_buffer: Vec<u8>,
}

impl Interface {
    pub fn allowed(&self) -> Sources {
        Sources::from_bits(self.cause.sources().to_bits() & self.mask.sources().to_bits())
    }

    pub fn write_cause(&mut self, new: InterruptCause) {
        self.cause = InterruptCause::from_bits(self.cause.to_bits() & !new.to_bits())
            .with_reset_state(self.cause.reset_state());
    }

    pub fn raise_interrupt(&mut self, interrupt: Interrupt) {
        let sources = self.cause.sources().to_bits().value() | (1 << interrupt as usize);
        self.cause
            .set_sources(Sources::from_bits(u14::new(sources)));
    }
}

impl System {
    pub fn check_external_interrupts(&mut self) {
        if !self.cpu.supervisor.config.msr.interrupts() {
            return;
        }

        let allowed = self.processor.allowed();
        if allowed.to_bits().value() != 0 {
            self.cpu.raise_exception(Exception::Interrupt);
        }
    }

    /// Pushes a value into the PI FIFO. Values are queued up until 32 bytes are available, then
    /// written all at once.
    ///
    /// If the CP FIFO is linked wth the PI FIFO, this will also schedule a CP update.
    pub fn pi_fifo_push(&mut self, value: u8) {
        self.processor.fifo_buffer.push(value);
        if self.processor.fifo_buffer.len() < 32 {
            return;
        }

        tracing::debug!("flushing PI fifo");

        let data = std::mem::replace(&mut self.processor.fifo_buffer, Vec::with_capacity(32));
        for byte in data {
            self.write(self.processor.fifo_current, byte);
            self.processor.fifo_current += 1;

            if self.gpu.command.control.linked_mode() {
                self.gpu.command.fifo_push();
            }

            if self.processor.fifo_current > self.processor.fifo_end {
                self.processor.fifo_current = self.processor.fifo_start;
            }
        }

        if self.gpu.command.control.linked_mode() {
            self.scheduler.schedule(Event::CommandProcessor, 0);
        }
    }
}
