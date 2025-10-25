//! State of the emulator.

pub mod audio;
pub mod bus;
pub mod disk;
pub mod dsp;
pub mod eabi;
pub mod executable;
pub mod external;
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
use dol::binrw::BinRead;
use iso::Iso;
use std::io::{Cursor, Read, Seek};

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
    /// The external interface.
    pub external: external::Interface,
    /// The audio interface.
    pub audio: audio::Interface,
    /// The disk interface.
    pub disk: disk::Interface,
}

impl System {
    fn load_apploader(&mut self) -> Option<Address> {
        let Some(iso) = &mut self.config.iso else {
            return None;
        };

        let apploader = iso.apploader().unwrap();
        let size = apploader.size;
        self.mem.ram[0x0120_0000..][..size as usize].copy_from_slice(&apploader.data);

        Some(Address(apploader.entrypoint))
    }

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

    fn load_iso(&mut self) {
        self.cpu.supervisor.memory.setup_default_bats();
        self.mmu.build_bat_lut(&self.cpu.supervisor.memory);

        // load apploader
        let entry = self.load_apploader().unwrap();

        // load fake-ipl
        let mut cursor = Cursor::new(include_bytes!("../../resources/fake-ipl.dol"));
        let ipl = dol::Dol::read(&mut cursor).unwrap();
        self.config.executable = Some(Executable::new(Code::Dol(ipl)));
        self.load_executable();

        // setup apploader entrypoint for fake-ipl
        self.cpu.user.gpr[3] = entry.value();

        // load dolphin-os constans
        self.write::<u32>(Address(0x20), 0x0D15EA5E); // Boot kind
        self.write::<u32>(Address(0x24), 0x1); // Version
        self.write::<u32>(Address(0x28), 0x01800000); // Physical Memory Size
        self.write::<u32>(Address(0x2C), 0x00000001); // Console Type
        self.write::<u32>(Address(0x30), 0x00000000); // Arena Low
        self.write::<u32>(Address(0x34), 0x817fe8c0); // Arena High
        self.write::<u32>(Address(0x38), 0x817fe8c0); // FST address
        self.write::<u32>(Address(0x3C), 0x24); // FST max length
        self.write::<u32>(Address(0xD0), 16 * 1024 * 1024); // ARAM size

        // setup MSR
        self.cpu.supervisor.config.msr.set_exception_prefix(false);

        // done :)
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
            external: external::Interface::default(),
            audio: audio::Interface::default(),
            disk: disk::Interface::default(),

            config,
        };

        if system.config.iso.is_some() {
            system.load_iso();
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
