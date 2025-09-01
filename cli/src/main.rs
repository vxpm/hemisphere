mod app;

use binrw::io::BufReader;
use clap::Parser;
use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, Hemisphere,
    dolfile::{Dol, binrw::BinRead},
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
    /// Whether to start running right away
    #[arg(short, long, default_value_t = false)]
    run: bool,
}

fn setup_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    let file = std::fs::File::options()
        .truncate(true)
        .create(true)
        .write(true)
        .open("log.txt")
        .unwrap();

    let (file, _guard_file) = tracing_appender::non_blocking(file);
    // let (stderr, _guard_stderr) = tracing_appender::non_blocking(std::io::stderr());

    let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new(
        "cli=trace,hemisphere=trace,hemicore=trace,ppcjit=trace",
    ));

    let file_layer = fmt::layer().with_writer(file).with_ansi(false);
    // let stderr_layer = fmt::layer().with_writer(stderr).with_ansi(true);

    let subscriber = tracing_subscriber::registry()
        .with(file_layer)
        // .with(stderr_layer)
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

    let mut runner = Runner::new(Hemisphere::new(Config::default()));
    runner.with_state(|state| state.hemisphere_mut().system.load(&dol));
    runner.set_run(args.run);

    let mut app = App::new(runner);

    info!("starting interface");
    let terminal = ratatui::init();
    app.run(terminal)?;
    ratatui::restore();

    Ok(())
}
