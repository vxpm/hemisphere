//! Processor interface.

use crate::system::{Event, System};
use bitos::{bitos, integer::u26};
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
pub struct InterruptSources {
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
    pub sources: InterruptSources,
}

#[bitos(32)]
#[derive(Default, Debug, Clone, Copy)]
pub struct FifoCurrent {
    #[bits(0..26)]
    pub base: u26,
    #[bits(29)]
    pub wrapped: bool,
}

impl FifoCurrent {
    pub fn address(&self) -> Address {
        Address(self.base().value())
    }

    pub fn set_address(&mut self, value: Address) {
        self.set_base(u26::new(value.value()));
    }
}

#[derive(Default)]
pub struct Interface {
    // interrupts
    pub mask: InterruptMask,

    // fifo
    pub fifo_start: Address,
    pub fifo_end: Address,
    pub fifo_current: FifoCurrent,
    pub fifo_buffer: Vec<u8>,
}

impl System {
    /// Returns which interrupt sources are active (i.e. triggered but maybe masked).
    pub fn get_active_interrupts(&self) -> InterruptSources {
        let mut sources = InterruptSources::default();

        let mut video = false;
        for i in &self.video.interrupts {
            video |= i.enable() && i.status();
        }
        sources.set_video_interface(video);

        sources.set_pe_token(self.gpu.pixel.interrupt.token());
        sources.set_pe_finish(self.gpu.pixel.interrupt.finish());
        sources.set_audio_interface(self.audio.control.interrupt());

        sources
    }

    /// Returns which interrupt sources are raised (i.e. triggered and unmasked).
    pub fn get_raised_interrupts(&self) -> InterruptSources {
        InterruptSources::from_bits(
            self.get_active_interrupts().to_bits() & self.processor.mask.sources().to_bits(),
        )
    }

    /// Checks whether any of the currently raised interrutps can be taken and, if any, raises the
    /// interrupt exception.
    pub fn check_interrupts(&mut self) {
        if !self.cpu.supervisor.config.msr.interrupts() {
            return;
        }

        let raised = self.get_raised_interrupts();
        if raised.to_bits().value() != 0 {
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

        let data = std::mem::replace(&mut self.processor.fifo_buffer, Vec::with_capacity(32));
        for byte in data {
            let current = self.processor.fifo_current.address();
            self.write(current, byte);
            self.processor.fifo_current.set_address(current + 1);

            if self.gpu.command.control.linked_mode() {
                self.gpu.command.fifo_push();
            }

            if self.processor.fifo_current.address() > self.processor.fifo_end + 4 {
                self.processor.fifo_current.set_wrapped(true);
                self.processor
                    .fifo_current
                    .set_address(self.processor.fifo_start);
            }
        }

        if self.gpu.command.control.linked_mode() {
            self.scheduler.schedule_now(Event::CommandProcessor);
        }
    }
}
