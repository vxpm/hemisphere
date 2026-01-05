#![feature(trim_prefix_suffix)]

mod cli;
mod runner;

mod control;
mod debug;
mod disasm;
mod efb;
mod registers;
mod renderer_info;
mod subsystem;
mod variables;
mod windows;
mod xfb;

use crate::{runner::Runner, windows::AppWindow};
use clap::Parser;
use eframe::{
    egui,
    egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew},
};
use eyre_pretty::eyre::Result;
use hemisphere::{
    Hemisphere,
    cores::Cores,
    modules::debug::{DebugModule, NopDebugModule},
    system::{self, Modules, executable::Executable},
};
use nanorand::Rng;
use renderer::Renderer;
use runner::State;
use serde::{Deserialize, Serialize};
use std::{
    io::BufReader,
    sync::Arc,
    time::{Duration, Instant},
};
use vtxjit::JitVertexModule;

use app_modules::{
    audio::CpalAudio,
    debug::{Addr2LineDebug, MapFileDebug},
    disk::IsoDisk,
    input::GilrsInput,
};
use cores::cpu::jit as jitcore;
use cores::dsp::interpreter as dspcore;

struct Ctx<'a> {
    step: bool,
    running: bool,
    renderer: &'a mut Renderer,
}

#[derive(Serialize, Deserialize)]
struct AppWindowState {
    id: egui::Id,
    open: bool,
    window: Box<dyn AppWindow>,
}

struct App {
    last_update: Instant,
    renderer: Renderer,
    windows: Vec<AppWindowState>,
    runner: Runner,
    cps: u64,
}

impl App {
    #[allow(clippy::default_constructed_unit_structs)]
    fn new(cc: &eframe::CreationContext<'_>, args: &cli::Args) -> Result<Self> {
        tracing::info!("loading executable");

        let ipl = if let Some(path) = &args.ipl {
            Some(std::fs::read(path)?)
        } else {
            None
        };

        let iso = IsoDisk(if let Some(path) = &args.iso {
            let file = std::fs::File::open(path)?;
            let reader = BufReader::new(file);
            Some(reader)
        } else {
            None
        });

        let executable = if let Some(path) = &args.exec {
            Some(Executable::open(path)?)
        } else {
            None
        };

        // this is a mess lol
        let debug_module = if let Some(path) = args.debug.as_deref() {
            match path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_ascii_lowercase())
                .as_deref()
            {
                Some("elf") => {
                    let debug = Addr2LineDebug::new(path);
                    debug.map_or_else(
                        || Box::new(NopDebugModule) as Box<dyn DebugModule>,
                        |d| Box::new(d) as Box<dyn DebugModule>,
                    )
                }
                Some("map") => Box::new(MapFileDebug::new(path)) as Box<dyn DebugModule>,
                _ => Box::new(NopDebugModule),
            }
        } else {
            Box::new(NopDebugModule)
        };

        let wgpu_state = cc.wgpu_render_state.as_ref().unwrap();
        tracing::info!("wgpu device limits: {:?}", wgpu_state.device.limits());

        let renderer = Renderer::new(
            wgpu_state.device.clone(),
            wgpu_state.queue.clone(),
            wgpu_state.target_format,
        );

        let dirs = directories::ProjectDirs::from("", "", "hemisphere").unwrap();
        let cache_dir = dirs.cache_dir();
        let jit_cache_path = cache_dir.join("ppcjit");

        if args.clear_cache {
            _ = std::fs::remove_dir_all(&jit_cache_path);
        }

        let cores = Cores {
            cpu: Box::new(jitcore::JitCore::new(jitcore::Config {
                instr_per_block: args.instr_per_block,
                jit_settings: jitcore::ppcjit::Settings {
                    compiler: jitcore::ppcjit::CompilerSettings {
                        nop_syscalls: args.nop_syscalls,
                        force_fpu: args.force_fpu,
                        ignore_unimplemented: args.ignore_unimplemented_instr,
                    },
                    cache_path: jit_cache_path,
                },
            })),
            dsp: Box::new(dspcore::InterpreterCore::default()),
        };

        let modules = Modules {
            audio: Box::new(CpalAudio::new()),
            debug: debug_module,
            disk: Box::new(iso),
            input: Box::new(GilrsInput::new()),
            render: Box::new(renderer.clone()),
            vertex: Box::new(JitVertexModule::new()),
        };

        let hemisphere = Hemisphere::new(
            cores,
            modules,
            system::Config {
                force_ipl: args.force_ipl,
                ipl,
                sideload: executable,
            },
        );

        let mut runner = runner::Runner::new(hemisphere);
        if args.run {
            runner.start();
        }

        let windows: Vec<AppWindowState> = cc
            .storage
            .as_ref()
            .and_then(|s| s.get_string("windows"))
            .and_then(|s| ron::from_str(&s).ok())
            .unwrap_or_default();

        cc.egui_ctx.set_zoom_factor(1.0);
        Ok(Self {
            last_update: Instant::now(),
            renderer,
            windows,
            runner,
            cps: 0,
        })
    }

    fn create_window(&mut self, window: impl AppWindow) {
        let mut rng = nanorand::tls_rng();
        let id = rng.generate::<u64>();
        self.windows.push(AppWindowState {
            id: egui::Id::new(id),
            open: true,
            window: Box::new(window),
        });
    }
}

const FRAMETIME: Duration = Duration::new(0, (1_000_000_000.0 / 60.0) as u32);

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.label("Hemisphere");
                ui.menu_button("🗖 View", |ui| {
                    if ui.button("Control").clicked() {
                        self.create_window(control::Window::default());
                    }

                    if ui.button("Disassembly").clicked() {
                        self.create_window(disasm::Window::default());
                    }

                    if ui.button("Registers").clicked() {
                        self.create_window(registers::Window::default());
                    }

                    if ui.button("Call Stack").clicked() {
                        self.create_window(debug::Window::default());
                    }

                    if ui.button("Variables").clicked() {
                        self.create_window(variables::Window::default());
                    }

                    if ui.button("XFB").clicked() {
                        self.create_window(xfb::Window::default());
                    }

                    if ui.button("EFB").clicked() {
                        self.create_window(efb::Window);
                    }

                    if ui.button("Renderer").clicked() {
                        self.create_window(renderer_info::Window::default());
                    }

                    ui.menu_button("Subsystems", |ui| {
                        if ui.button("Command Processor").clicked() {
                            self.create_window(subsystem::cp::Window::default());
                        }

                        if ui.button("Processor Interface").clicked() {
                            self.create_window(subsystem::pi::Window::default());
                        }
                    });
                });

                ui.label(format!(
                    "Speed: {}%",
                    ((self.cps as f64 / hemisphere::gekko::FREQUENCY as f64) * 100.0).round()
                ));
            });
        });

        let running = self.runner.running();
        self.runner.stop();

        {
            let mut state = self.runner.get();
            for window_state in &mut self.windows {
                window_state.window.prepare(&mut state);
            }

            self.cps = state
                .cycles_history
                .iter()
                .map(|c| c.0.value())
                .sum::<u64>()
                * 2;
        }

        if running {
            self.runner.start();
        }

        let mut context = Ctx {
            step: false,
            running,
            renderer: &mut self.renderer,
        };

        egui::CentralPanel::default().show(ctx, |_| {
            let mut close = None;
            for (index, window_state) in self.windows.iter_mut().enumerate() {
                let mut open = true;
                egui::Window::new(window_state.window.title())
                    .id(window_state.id)
                    .open(&mut open)
                    .resizable(true)
                    .min_size(egui::Vec2::ZERO)
                    .show(ctx, |ui| {
                        window_state.window.show(ui, &mut context);
                    });

                if !open {
                    close = Some(index);
                }
            }

            if let Some(close) = close {
                self.windows.remove(close);
            }
        });

        if context.running != running {
            if context.running {
                self.runner.start();
            } else {
                self.runner.stop();
            }
        }

        if context.step {
            self.runner.step();
        }

        let remaining = FRAMETIME.saturating_sub(self.last_update.elapsed());
        ctx.request_repaint_after(remaining);
        self.last_update = Instant::now() + remaining;
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let windows = self.windows.iter().collect::<Vec<_>>();
        storage.set_string("windows", ron::to_string(&windows).unwrap());
    }
}

fn setup_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    let file = std::fs::File::options()
        .truncate(true)
        .create(true)
        .write(true)
        .open("log.log")
        .unwrap();

    let (file_nb, _guard_file) = tracing_appender::non_blocking(file);
    let file_layer = fmt::layer().with_writer(file_nb).with_ansi(false);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new(
        "cli=debug,hemisphere=debug,hemisphere::system::gx=info,common=debug,ppcjit=debug,renderer=debug,dspint=debug,cores=debug",
    ));

    let subscriber = tracing_subscriber::registry()
        .with(file_layer)
        .with(env_filter);

    subscriber.init();

    _guard_file
}

fn main() -> Result<()> {
    eyre_pretty::install()?;
    let _tracing_guard = setup_tracing();

    let args = cli::Args::parse();

    let device_descriptor = Arc::new(|adapter: &wgpu::Adapter| {
        let base_limits = if adapter.get_info().backend == wgpu::Backend::Gl {
            wgpu::Limits::downlevel_defaults()
        } else {
            wgpu::Limits::defaults()
        };

        wgpu::DeviceDescriptor {
            label: Some("hemisphere-egui wgpu device"),
            required_features: wgpu::Features::DUAL_SOURCE_BLENDING
                | wgpu::Features::FLOAT32_FILTERABLE,
            required_limits: wgpu::Limits {
                // required by egui
                max_texture_dimension_2d: 8192,
                ..base_limits
            },
            ..Default::default()
        }
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        wgpu_options: WgpuConfiguration {
            wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
                instance_descriptor: wgpu::InstanceDescriptor {
                    // flags: wgpu::InstanceFlags::debugging(),
                    backends: wgpu::Backends::PRIMARY,
                    ..Default::default()
                },
                power_preference: wgpu::PowerPreference::HighPerformance,
                device_descriptor,
                ..Default::default()
            }),
            ..Default::default()
        },

        ..Default::default()
    };

    eframe::run_native(
        "Hemisphere",
        options,
        Box::new(|cc| {
            let app = App::new(cc, &args)?;
            Ok(Box::new(app))
        }),
    )?;

    Ok(())
}
