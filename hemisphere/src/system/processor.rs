//! Processor interface.

use crate::{
    Primitive,
    system::{System, gx},
};
use bitos::{bitos, integer::u26};
use gekko::{Address, Exception};
use std::collections::VecDeque;

#[bitos(14)]
#[derive(Default, Clone, Copy)]
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

impl std::fmt::Debug for InterruptSources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut set = f.debug_set();
        macro_rules! debug {
            ($($ident:ident),*) => {
                $(
                    if self.$ident() {
                        set.entry(&stringify!($ident));
                    }
                )*
            };
        }

        debug! {
            gp_error,
            reset,
            dvd_interface,
            serial_interface,
            external_interface,
            audio_interface,
            dsp_interface,
            memory_interface,
            video_interface,
            pe_token,
            pe_finish,
            command_processor,
            debug,
            high_speed_port
        }

        set.finish_non_exhaustive()
    }
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

pub struct Interface {
    // interrupts
    pub mask: InterruptMask,

    // fifo
    pub fifo_start: Address,
    pub fifo_end: Address,
    pub fifo_current: FifoCurrent,
    pub fifo_buffer: Option<VecDeque<u8>>,
}

impl Default for Interface {
    fn default() -> Self {
        Self {
            mask: Default::default(),
            fifo_start: Default::default(),
            fifo_end: Default::default(),
            fifo_current: Default::default(),
            fifo_buffer: Some(VecDeque::with_capacity(32)),
        }
    }
}

impl System {
    /// Returns which interrupt sources are active (i.e. triggered but maybe masked).
    pub fn get_active_interrupts(&self) -> InterruptSources {
        let mut sources = InterruptSources::default();

        // VI
        let mut video = false;
        for i in &self.video.interrupts {
            video |= i.enable() && i.status();
        }
        sources.set_video_interface(video);

        // PE
        sources.set_pe_token(self.gpu.pixel.interrupt.token());
        sources.set_pe_finish(self.gpu.pixel.interrupt.finish());

        // AI
        sources.set_audio_interface(self.audio.control.interrupt());

        // DSP
        sources.set_dsp_interface(self.dsp.control.any_interrupt());

        // DI
        sources.set_dvd_interface(self.disk.status.any_interrupt());

        // SI
        sources.set_serial_interface(self.serial.any_interrupt());

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
    pub fn pi_check_interrupts(&mut self) {
        if !self.cpu.supervisor.config.msr.interrupts() {
            return;
        }

        let raised = self.get_raised_interrupts();
        if raised.to_bits().value() != 0 {
            tracing::debug!("raising interrupt exception for {raised:?}");
            self.cpu.raise_exception(Exception::Interrupt);
        }
    }

    /// Pushes a value into the PI FIFO. Values are queued up until 32 bytes are available, then
    /// written all at once.
    pub fn pi_fifo_push<P: Primitive>(&mut self, value: P) {
        let Some(mut fifo_buffer) = self.processor.fifo_buffer.take() else {
            unreachable!()
        };

        let mut buf = [0; 4];
        value.write_be_bytes(&mut buf);
        fifo_buffer.extend(&buf[..size_of::<P>()]);

        if fifo_buffer.len() < 32 {
            self.processor.fifo_buffer = Some(fifo_buffer);
            return;
        }

        let data = fifo_buffer.drain(..32);
        for byte in data {
            let current = self.processor.fifo_current.address();
            self.write(current, byte);
            self.processor.fifo_current.set_address(current + 1);
            if self.processor.fifo_current.address() > self.processor.fifo_end {
                std::hint::cold_path();
                self.processor.fifo_current.set_wrapped(true);
                self.processor
                    .fifo_current
                    .set_address(self.processor.fifo_start);
            }
        }

        if self.gpu.command.control.linked_mode() {
            gx::command::sync_to_pi(self);
            gx::command::consume(self);
        }

        self.processor.fifo_buffer = Some(fifo_buffer);
    }
}
