#![feature(trim_prefix_suffix)]

mod cli;
mod control;
mod debug;
mod disasm;
mod subsystem;
mod xfb;

use clap::Parser;
use eframe::{
    egui,
    egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew},
};
use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, jit,
    runner::{Runner, State},
    system::{self, executable::Executable},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

struct Ctx {
    step: bool,
    running: bool,
}

trait WindowUi {
    fn title(&self) -> &str;
    fn show(&mut self, ui: &mut egui::Ui, ctx: &mut Ctx, state: &mut State);
}

struct Window {
    open: bool,
    ui: Box<dyn WindowUi>,
}

impl Window {
    pub fn new(ui: impl WindowUi + 'static) -> Self {
        Self {
            open: true,
            ui: Box::new(ui),
        }
    }
}

struct WindowGroup {
    name: &'static str,
    windows: Vec<Window>,
    groups: Vec<WindowGroup>,
}

struct App {
    last_update: Instant,
    runner: Runner,
    vsync_count: Arc<AtomicU64>,
    root: WindowGroup,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, args: &cli::Args) -> Result<Self> {
        tracing::info!("loading executable");
        let dwarf = match args.dwarf.as_deref() {
            Some(debug) => Some(debug.to_path_buf()),
            None => {
                let mut elf = args.input.clone();
                elf.set_extension("elf");
                elf.exists().then_some(elf)
            }
        };
        let executable = Executable::open(&args.input, dwarf.as_deref())?;

        let vsync_count = Arc::new(AtomicU64::new(0));
        let vsync_callback = {
            let vsync_count = vsync_count.clone();
            Box::new(move || {
                vsync_count.fetch_add(1, Ordering::Relaxed);
            })
        };

        let mut runner = Runner::new(Config {
            system: system::Config {
                executable: Some(executable),
                vsync_callback: Some(vsync_callback),
            },
            jit: jit::Config {
                instr_per_block: args.instr_per_block,
            },
        });
        runner.set_run(args.run);

        let subsystem = WindowGroup {
            name: "Subsystems",
            windows: vec![Window::new(subsystem::cp::Window::default())],
            groups: vec![],
        };

        let root = WindowGroup {
            name: "root",
            windows: vec![
                // xfb
                Window::new(xfb::Window::new()),
                // cpu
                Window::new(control::Window::default()),
                Window::new(disasm::Window::default()),
                // debug
                Window::new(debug::Window::default()),
            ],
            groups: vec![subsystem],
        };

        cc.egui_ctx.set_zoom_factor(1.0);
        Ok(Self {
            last_update: Instant::now(),
            runner,
            vsync_count,
            root,
        })
    }
}

fn display_group(ui: &mut egui::Ui, group: &mut WindowGroup) {
    for window in &mut group.windows {
        let button = egui::Button::new(window.ui.title()).selected(window.open);
        if ui.add(button).clicked() {
            window.open = !window.open;
        }
    }

    for group in &mut group.groups {
        ui.menu_button(group.name, |ui| {
            display_group(ui, group);
        });
    }
}

fn display_windows(
    egui_ctx: &egui::Context,
    max_rect: egui::Rect,
    current_id: &mut u32,
    context: &mut Ctx,
    state: &mut State,
    group: &mut WindowGroup,
) {
    for win in &mut group.windows {
        let widget = egui::Window::new(win.ui.title())
            .open(&mut win.open)
            .id(egui::Id::new(*current_id))
            .resizable(true)
            .min_size(egui::Vec2::ZERO);

        *current_id += 1;

        widget
            .constrain_to(max_rect)
            .show(egui_ctx, |ui| win.ui.show(ui, context, state));
    }

    for group in &mut group.groups {
        display_windows(egui_ctx, max_rect, current_id, context, state, group);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.label("Hemisphere");
                ui.menu_button("🗖 View", |ui| {
                    display_group(ui, &mut self.root);
                });
            });
        });

        let running = self.runner.running();
        let mut context = Ctx {
            step: false,
            running,
        };

        self.runner.with_state(|state| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut id = 0;
                display_windows(
                    ctx,
                    ui.max_rect(),
                    &mut id,
                    &mut context,
                    state,
                    &mut self.root,
                );
            });
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
        "cli=debug,hemisphere=debug,common=debug,ppcjit=debug",
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        wgpu_options: WgpuConfiguration {
            wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
                instance_descriptor: wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::all(),
                    ..Default::default()
                },
                power_preference: wgpu::PowerPreference::HighPerformance,
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
