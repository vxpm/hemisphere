#![feature(trim_prefix_suffix)]

mod cli;
mod control;
mod debug;
mod disasm;
mod efb;
mod registers;
mod subsystem;
mod windows;
mod xfb;

use crate::windows::AppWindow;
use addr2line::gimli;
use clap::Parser;
use eframe::{
    egui,
    egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew},
};
use eyre_pretty::{ContextCompat, bail, eyre::Result};
use hemisphere::{
    Address, Config, iso, jit,
    runner::Runner,
    system::{
        self,
        executable::{DebugInfo, Executable, Location},
    },
};
use nanorand::Rng;
use renderer::WgpuRenderer;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    io::BufReader,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

struct Addr2LineDebug(addr2line::Loader);

impl DebugInfo for Addr2LineDebug {
    fn find_symbol(&self, addr: Address) -> Option<String> {
        self.0
            .find_symbol(addr.value() as u64)
            .map(|s| addr2line::demangle_auto(Cow::Borrowed(s), Some(gimli::DW_LANG_C_plus_plus)))
            .map(|s| s.into_owned())
    }

    fn find_location(&self, addr: Address) -> Option<Location<'_>> {
        self.0
            .find_location(addr.value() as u64)
            .ok()
            .flatten()
            .map(|l| Location {
                file: l.file,
                line: l.line,
                column: l.column,
            })
    }
}

struct Ctx<'a> {
    step: bool,
    running: bool,
    renderer: &'a mut WgpuRenderer,
}

#[derive(Serialize, Deserialize)]
struct AppWindowState {
    id: egui::Id,
    open: bool,
    window: Box<dyn AppWindow>,
}

struct App {
    last_update: Instant,
    renderer: WgpuRenderer,
    runner: Runner,
    vsync_count: Arc<AtomicU64>,
    windows: Vec<AppWindowState>,
}

impl App {
    #[allow(clippy::default_constructed_unit_structs)]
    fn new(cc: &eframe::CreationContext<'_>, args: &cli::Args) -> Result<Self> {
        tracing::info!("loading executable");

        let input_extension = args
            .input
            .extension()
            .and_then(|e| e.to_str())
            .context("unknown input file extension")?;

        let mut ipl = None;
        if let Some(path) = &args.ipl {
            ipl = Some(std::fs::read(path)?);
        }

        let mut iso = None;
        let mut executable = None;
        match input_extension {
            "iso" => {
                let file = std::fs::File::open(&args.input)?;
                let reader = BufReader::new(file);
                iso = Some(iso::Iso::new(Box::new(reader) as _)?);
            }
            "dol" => {
                executable = Some(Executable::open(&args.input)?);
            }
            _ => bail!("unknown input file extension"),
        }

        let debug_info = args
            .dwarf
            .as_deref()
            .and_then(|p| addr2line::Loader::new(p).ok())
            .map(|l| Box::new(Addr2LineDebug(l)) as _);

        let vsync_count = Arc::new(AtomicU64::new(0));
        let vsync_callback = {
            let vsync_count = vsync_count.clone();
            Box::new(move || {
                vsync_count.fetch_add(1, Ordering::Relaxed);
            })
        };

        let wgpu_state = cc.wgpu_render_state.as_ref().unwrap();
        tracing::info!("wgpu device limits: {:?}", wgpu_state.device.limits());

        let renderer = WgpuRenderer::new(
            wgpu_state.device.clone(),
            wgpu_state.queue.clone(),
            wgpu_state.target_format,
        );

        let mut runner = Runner::new(Config {
            system: system::Config {
                renderer: Box::new(renderer.clone()),
                ipl,
                iso,
                sideload: executable,
                debug_info,
                vsync_callback: Some(vsync_callback),
            },
            jit: jit::Config {
                instr_per_block: args.instr_per_block,
            },
        });
        runner.set_run(args.run);

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
            runner,
            vsync_count,
            windows,
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

                    if ui.button("XFB").clicked() {
                        self.create_window(xfb::Window::default());
                    }

                    if ui.button("EFB").clicked() {
                        self.create_window(efb::Window::default());
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
            });
        });

        self.runner.with_state(|state| {
            for window_state in &mut self.windows {
                window_state.window.prepare(state);
            }
        });

        let running = self.runner.running();
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
            self.runner.set_run(context.running);
        }

        if context.step {
            self.runner.step();
        }

        const FRAMETIME: Duration = Duration::new(0, (1_000_000_000.0 / 60.0) as u32);
        let vsyncs = self.vsync_count.load(Ordering::Relaxed);
        loop {
            if self.last_update.elapsed() >= FRAMETIME
                || vsyncs < self.vsync_count.load(Ordering::Relaxed)
            {
                break;
            }

            std::thread::yield_now();
        }

        ctx.request_repaint();
        self.last_update = Instant::now();
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
        "cli=debug,hemisphere=debug,common=debug,ppcjit=debug,renderer=debug",
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
            wgpu::Limits::default()
        };

        wgpu::DeviceDescriptor {
            label: Some("hemisphere-egui wgpu device"),
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
                    backends: wgpu::Backends::all(),
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
