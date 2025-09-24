mod cli;
mod xfb;

use std::time::Duration;

use clap::Parser;
use eframe::{
    egui,
    egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew},
};
use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, Hemisphere, jit,
    runner::{Runner, State},
    system::{self, executable::Executable},
};
use tracing::info;

trait WindowUi {
    fn build<'open>(&mut self, open: &'open mut bool) -> egui::Window<'open>;
    fn show(&mut self, ui: &mut egui::Ui, state: &mut State);
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

struct App {
    runner: Runner,
    windows: Vec<Window>,
}

impl App {
    fn new(_: &eframe::CreationContext<'_>, runner: Runner) -> Self {
        // TODO: setup renderer with WGPU (cc.wgpu_render_state)

        Self {
            runner,
            windows: vec![Window::new(xfb::Window::default())],
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let start = std::time::Instant::now();

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.label("Hemisphere");
                ui.menu_button("test", |_| {});
            });
        });

        self.runner.with_state(|state| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for win in &mut self.windows {
                    let widget = win.ui.build(&mut win.open);
                    widget
                        .constrain_to(ui.max_rect())
                        .show(ctx, |ui| win.ui.show(ui, state));
                }
            });
        });

        ctx.request_repaint(
            // Duration::from_secs_f64(1.0 / 60.0).saturating_sub(start.elapsed()),
        );
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
    let args = cli::Args::parse();
    let _tracing_guard = setup_tracing();

    tracing::info!("loading executable");
    let dwarf = match args.dwarf {
        Some(debug) => Some(debug),
        None => {
            let mut elf = args.input.clone();
            elf.set_extension("elf");
            elf.exists().then_some(elf)
        }
    };

    let executable = Executable::open(&args.input, dwarf.as_deref())?;
    let mut runner = Runner::new(Hemisphere::new(Config {
        system: system::Config {
            executable: Some(executable),
        },
        jit: jit::Config {
            instr_per_block: args.instr_per_block,
        },
    }));

    runner.set_run(args.run);

    let mut options = eframe::NativeOptions::default();
    options.wgpu_options = WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
            instance_descriptor: wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            },
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }),
        ..Default::default()
    };

    info!("starting interface");
    eframe::run_native(
        "Hemisphere",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc, runner)))),
    )?;

    Ok(())
}
