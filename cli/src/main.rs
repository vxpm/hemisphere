mod app;

use clap::Parser;
use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, Hemisphere, jit,
    runner::Runner,
    system::{self, executable::Executable},
};
use ratatui::crossterm;
use std::path::PathBuf;
use tracing::info;

use crate::app::App;

/// Hemisphere: GameCube emulator
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    /// Path to a .dol file to load and execute
    input: PathBuf,
    #[arg(long)]
    dwarf: Option<PathBuf>,
    /// Whether to start running right away
    #[arg(short, long, default_value_t = false)]
    run: bool,
    /// Maximum number of instructions per block
    #[arg(visible_alias("ipb"), long, default_value_t = 256)]
    instr_per_block: u32,
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
    let args = CliArgs::parse();
    let _tracing_guard = setup_tracing();

    info!("loading executable");

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
    let mut app = App::new(runner);

    // setup panic hook to restore terminal first
    // NOTE: don't use ratatui::init as it replaces the panic hook
    hemisphere::panic::set_hook(
        Box::new(move |_| {
            ratatui::restore();
        }),
        true,
    );

    info!("starting interface");
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
    let terminal = ratatui::Terminal::new(backend)?;

    app.run(terminal)?;
    ratatui::restore();

    Ok(())
}
