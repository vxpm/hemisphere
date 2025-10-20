//! State of the emulator.

pub mod bus;
pub mod dsp;
pub mod eabi;
pub mod executable;
pub mod gpu;
pub mod lazy;
pub mod mem;
pub mod mmu;
pub mod processor;
pub mod scheduler;
pub mod video;

use crate::{
    render::Renderer,
    system::{
        dsp::Dsp,
        executable::{Code, Executable},
        gpu::Gpu,
        lazy::Lazy,
        mem::Memory,
        mmu::Mmu,
        scheduler::Scheduler,
    },
};
use common::{
    Address,
    arch::{Cpu, Exception, FREQUENCY},
};
use iso::Iso;
use std::io::{Read, Seek};

pub type Callback = Box<dyn FnMut() + Send + Sync + 'static>;

pub trait ReadAndSeek: Read + Seek + Send + 'static {}
impl<T> ReadAndSeek for T where T: Read + Seek + Send + 'static {}

/// System configuration.
pub struct Config {
    pub renderer: Box<dyn Renderer>,
    pub ipl: Option<Vec<u8>>,
    pub iso: Option<Iso<Box<dyn ReadAndSeek>>>,
    pub executable: Option<Executable>,
    pub vsync_callback: Option<Callback>,
}

/// An event which can be scheduled to happen at a specific time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Decrementer has underflowed.
    Decrementer,
    /// Check external interrupts.
    CheckInterrupts,
    /// Process commands in the CP FIFO.
    CommandProcessor,
    /// A video interface event.
    Video(video::Event),
}

/// System state.
pub struct System {
    /// System configuration.
    pub config: Config,
    /// Scheduler for events.
    pub scheduler: Scheduler,
    /// The CPU state.
    pub cpu: Cpu,
    /// The GPU state.
    pub gpu: Gpu,
    /// The DSP state.
    pub dsp: Dsp,
    /// System memory.
    pub mem: Memory,
    /// State of memory mapping.
    pub mmu: Mmu,
    /// State of mechanisms that update lazily (e.g. time related registers).
    pub lazy: Lazy,
    /// The video interface.
    pub video: video::Interface,
    /// The processor interface.
    pub processor: processor::Interface,
}

impl System {
    fn load_executable(&mut self) {
        let Some(exec) = self.config.executable.take() else {
            return;
        };

        match exec.code() {
            Code::Dol(dol) => {
                self.cpu.pc = Address(dol.entrypoint());
                self.cpu.supervisor.memory.setup_default_bats();
                self.mmu.build_bat_lut(&self.cpu.supervisor.memory);

                self.cpu
                    .supervisor
                    .config
                    .msr
                    .set_instr_addr_translation(true);
                self.cpu
                    .supervisor
                    .config
                    .msr
                    .set_data_addr_translation(true);

                // zero bss first, let other sections overwrite it if it occurs
                for offset in 0..dol.header.bss_size {
                    let target = self
                        .translate_data_addr(Address(dol.header.bss_target + offset))
                        .unwrap();
                    self.write(target, 0u8);
                }

                for section in dol.text_sections() {
                    for (offset, byte) in section.content.iter().copied().enumerate() {
                        let target = self
                            .translate_instr_addr(Address(section.target) + offset as u32)
                            .unwrap();
                        self.write(target, byte);
                    }
                }

                for section in dol.data_sections() {
                    for (offset, byte) in section.content.iter().copied().enumerate() {
                        let target = self
                            .translate_data_addr(Address(section.target) + offset as u32)
                            .unwrap();
                        self.write(target, byte);
                    }
                }
            }
        }

        self.config.executable = Some(exec);
        tracing::debug!("finished loading executable");
    }

    pub fn new(mut config: Config) -> Self {
        let mut system = System {
            scheduler: Scheduler::default(),
            cpu: Cpu::default(),
            gpu: Gpu::default(),
            dsp: Dsp::default(),
            mem: Memory::new(
                config
                    .ipl
                    .take()
                    .unwrap_or_else(|| vec![0; mem::IPL_LEN as usize]),
            ),
            mmu: Mmu::default(),
            lazy: Lazy::default(),
            video: video::Interface::default(),
            processor: processor::Interface::default(),

            config,
        };

        if let Some(_) = &mut system.config.iso {
            system.cpu.supervisor.memory.setup_default_bats();
            system.cpu.supervisor.config.msr.set_exception_prefix(true);
            system.mmu.build_bat_lut(&system.cpu.supervisor.memory);
            system.cpu.raise_exception(Exception::Reset);
        } else if system.config.executable.is_some() {
            system.load_executable();
        }

        system
    }

    /// Translates a data logical address into a physical address.
    pub fn translate_data_addr(&self, addr: Address) -> Option<Address> {
        if !self.cpu.supervisor.config.msr.data_addr_translation() {
            return Some(addr);
        }

        self.mmu.translate_data_addr(addr)
    }

    /// Translates an instruction logical address into a physical address.
    pub fn translate_instr_addr(&self, addr: Address) -> Option<Address> {
        if !self.cpu.supervisor.config.msr.instr_addr_translation() {
            return Some(addr);
        }

        self.mmu.translate_instr_addr(addr)
    }

    /// Processes the given event.
    pub fn process(&mut self, event: Event) {
        match event {
            Event::Decrementer => {
                self.update_decrementer();
                if self.cpu.supervisor.config.msr.interrupts() {
                    self.cpu.raise_exception(Exception::Decrementer);
                    self.scheduler.schedule(Event::Decrementer, u32::MAX as u64);
                } else {
                    self.scheduler.schedule(Event::Decrementer, 32);
                }
            }
            Event::CheckInterrupts => self.check_interrupts(),
            Event::CommandProcessor => self.cp_update(),
            Event::Video(video::Event::VerticalCount) => {
                self.update_display_interrupts();

                self.video.vertical_count += 1;
                if self.video.vertical_count as u32 > self.video.lines_per_frame() {
                    self.video.vertical_count = 1;
                    if let Some(callback) = &mut self.config.vsync_callback {
                        callback();
                    }
                }

                if !self.video.display_config.progressive()
                    && self.video.vertical_count as u32 == self.video.lines_per_even_field() + 1
                    && let Some(callback) = &mut self.config.vsync_callback
                {
                    callback();
                }

                let cycles_per_frame = (FREQUENCY as f64 / self.video.refresh_rate()) as u32;
                let cycles_per_line = cycles_per_frame
                    .checked_div(self.video.lines_per_frame())
                    .unwrap_or(cycles_per_frame);

                self.scheduler.schedule(
                    Event::Video(video::Event::VerticalCount),
                    cycles_per_line as u64,
                );
            }
        }
    }
}
