//! State of the system (i.e. GameCube and emulator).

pub mod bus;
pub mod eabi;
pub mod executable;
pub mod ipl;
pub mod lazy;
pub mod scheduler;

pub mod ai;
pub mod di;
pub mod dspi;
pub mod exi;
pub mod gx;
pub mod mem;
pub mod mmu;
pub mod pi;
pub mod si;
pub mod vi;

use crate::{
    modules::{
        audio::AudioModule, debug::DebugModule, disk::DiskModule, input::InputModule,
        render::RenderModule,
    },
    system::{
        dspi::Dsp,
        executable::Executable,
        gx::Gpu,
        ipl::Ipl,
        lazy::Lazy,
        mem::Memory,
        mmu::Mmu,
        scheduler::{HandlerCtx, Scheduler},
    },
};
use dol::binrw::BinRead;
use easyerr::{Error, ResultExt};
use gekko::{Address, Cpu, Cycles};
use std::io::{Cursor, SeekFrom};

/// System configuration.
pub struct Config {
    pub force_ipl: bool,
    pub ipl: Option<Vec<u8>>,
    pub sideload: Option<Executable>,
}

/// System modules.
pub struct Modules {
    pub audio: Box<dyn AudioModule>,
    pub debug: Box<dyn DebugModule>,
    pub disk: Box<dyn DiskModule>,
    pub input: Box<dyn InputModule>,
    pub render: Box<dyn RenderModule>,
}

/// System state.
pub struct System {
    /// System configuration.
    pub config: Config,
    /// System modules.
    pub modules: Modules,
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
    pub video: vi::Interface,
    /// The processor interface.
    pub processor: pi::Interface,
    /// The external interface.
    pub external: exi::Interface,
    /// The audio interface.
    pub audio: ai::Interface,
    /// The disk interface.
    pub disk: di::Interface,
    /// The serial interface.
    pub serial: si::Interface,
}

#[derive(Debug, Error)]
pub enum LoadApploaderError {
    #[error(transparent)]
    Io { source: std::io::Error },
    #[error(transparent)]
    Apploader { source: iso::binrw::Error },
}

impl System {
    fn load_apploader(&mut self) -> Result<Address, LoadApploaderError> {
        self.modules
            .disk
            .seek(SeekFrom::Start(0x2440))
            .context(LoadApploaderCtx::Io)?;

        let apploader =
            iso::Apploader::read(&mut self.modules.disk).context(LoadApploaderCtx::Apploader)?;

        let size = apploader.size;
        self.mem.ram_mut()[0x0120_0000..][..size as usize].copy_from_slice(&apploader.data);

        Ok(Address(apploader.entrypoint))
    }

    fn load_executable(&mut self) {
        let Some(exec) = self.config.sideload.take() else {
            return;
        };

        match &exec {
            Executable::Dol(dol) => {
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

        self.config.sideload = Some(exec);
        tracing::debug!("finished loading executable");
    }

    fn load_ipl_hle(&mut self) {
        self.cpu.supervisor.memory.setup_default_bats();
        self.mmu.build_bat_lut(&self.cpu.supervisor.memory);

        self.modules
            .disk
            .seek(SeekFrom::Start(0))
            .context(LoadApploaderCtx::Io)
            .unwrap();

        let header = iso::Header::read(&mut self.modules.disk)
            .context(LoadApploaderCtx::Apploader)
            .unwrap();

        tracing::info!(
            game_code = header.game_code(),
            maker_code = header.maker_code,
            disk_id = header.disk_id,
            version = header.version,
            audio_streaming = header.audio_streaming,
            stream_buffer_size = header.stream_buffer_size,
            "loading '{}' ({}) using IPL HLE",
            header.game_name,
            header.game_code_str().as_deref().unwrap_or("<unknown>")
        );

        // load apploader
        let entry = self.load_apploader().unwrap();

        // load ipl-hle
        let mut cursor = Cursor::new(include_bytes!("../../local/ipl-hle.dol"));
        let ipl = dol::Dol::read(&mut cursor).unwrap();
        self.config.sideload = Some(Executable::Dol(ipl));
        self.load_executable();

        // setup apploader entrypoint for ipl-hle
        self.cpu.user.gpr[3] = entry.value();

        // load dolphin-os globals
        self.write::<u32>(Address(0x00), header.game_code());
        self.write::<u16>(Address(0x04), header.maker_code);
        self.write::<u8>(Address(0x06), header.disk_id);
        self.write::<u8>(Address(0x07), header.version);
        self.write::<u8>(Address(0x08), header.audio_streaming);
        self.write::<u8>(Address(0x09), header.stream_buffer_size);

        self.write::<u32>(Address(0x1C), 0xC233_9F3D); // DVD Magic Word
        self.write::<u32>(Address(0x20), 0x0D15_EA5E); // Boot kind
        self.write::<u32>(Address(0x24), 0x0000_0001); // Version
        self.write::<u32>(Address(0x28), 0x0180_0000); // Physical Memory Size
        self.write::<u32>(Address(0x2C), 0x1000_0005); // Console Type
        self.write::<u32>(Address(0x30), 0x8042_E260); // Arena Low
        self.write::<u32>(Address(0x34), 0x817F_E8C0); // Arena High
        self.write::<u32>(Address(0x38), 0x817F_E8C0); // FST address
        self.write::<u32>(Address(0x3C), 0x0000_0024); // FST max length
        // TODO: deal with TV mode, games hang if it is wrong...
        self.write::<u32>(Address(0xCC), 0x0000_0000); // TV Mode
        self.write::<u32>(Address(0xD0), 0x0100_0000); // ARAM size
        self.write::<u32>(Address(0xF8), 0x09A7_EC80); // Bus clock
        self.write::<u32>(Address(0xFC), 0x1CF7_C580); // CPU clock

        self.video
            .display_config
            .set_video_format(vi::VideoFormat::Pal50);

        // setup MSR
        self.cpu.supervisor.config.msr.set_exception_prefix(false);

        // done :)
    }

    fn load_ipl(&mut self) {
        self.cpu.supervisor.config.msr.set_exception_prefix(true);
        self.cpu.pc = Address(0xFFF0_0100);
    }

    pub fn new(modules: Modules, mut config: Config) -> Self {
        let mut scheduler = Scheduler::default();
        scheduler.schedule(1 << 16, gx::cmd::process);

        let ipl = Ipl::new(
            config
                .ipl
                .take()
                .unwrap_or_else(|| vec![0; mem::IPL_LEN as usize]),
        );

        let mut system = System {
            scheduler,
            cpu: Cpu::default(),
            gpu: Gpu::default(),
            dsp: Dsp::new(),
            mem: Memory::new(&ipl),
            mmu: Mmu::default(),
            lazy: Lazy::default(),
            video: vi::Interface::default(),
            processor: pi::Interface::default(),
            external: exi::Interface::new(),
            audio: ai::Interface::default(),
            disk: di::Interface::default(),
            serial: si::Interface::default(),

            config,
            modules,
        };

        if system.config.force_ipl {
            system.load_ipl();
        } else if system.config.sideload.is_some() {
            system.load_executable();
        } else if system.modules.disk.has_disk() {
            system.load_ipl_hle();
        } else {
            system.load_ipl();
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

    /// Processes scheduled events.
    #[inline(always)]
    pub fn process_events(&mut self) {
        while let Some(event) = self.scheduler.pop() {
            let cycles_late = self.scheduler.elapsed() - event.cycle;
            let ctx = HandlerCtx {
                cycles_late: Cycles(cycles_late),
            };

            event.handler.call(self, ctx);
        }
    }
}
