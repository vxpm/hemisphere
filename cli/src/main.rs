mod app;

use binrw::io::BufReader;
use clap::Parser;
use eyre_pretty::eyre::{self, Result};
use hemisphere::{
    Config, Hemisphere,
    dol::{Dol, binrw::BinRead},
    runner::Runner,
};
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
    #[arg(visible_alias("ipb"), long, default_value_t = 128)]
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

    let (file, _guard_file) = tracing_appender::non_blocking(file);
    let file_layer = fmt::layer().with_writer(file).with_ansi(false);
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

    info!("loading {}", args.input.display());
    let file = std::fs::File::open(args.input).unwrap();
    let dol = Dol::read(&mut BufReader::new(file)).unwrap();

    let addr2line = match args
        .dwarf
        .map(|path| addr2line::Loader::new(path))
        .transpose()
    {
        Ok(dwarf) => dwarf,
        Err(e) => eyre::bail!("{e}"),
    };

    let mut runner = Runner::new(Hemisphere::new(Config {
        instr_per_block: args.instr_per_block,
    }));
    runner.with_state(|state| state.hemisphere_mut().system.load(&dol));
    runner.set_run(args.run);

    let mut app = App::new(runner, addr2line);

    info!("starting interface");
    let terminal = ratatui::init();
    app.run(terminal)?;
    ratatui::restore();

    Ok(())
}
