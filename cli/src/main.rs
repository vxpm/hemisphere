mod gdb;

use binrw::io::BufReader;
use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, Hemisphere,
    dolfile::{Dol, binrw::BinRead},
};
use tracing::info;

struct App {
    hemisphere: Hemisphere,
}

fn setup_tracing() -> [tracing_appender::non_blocking::WorkerGuard; 2] {
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    let file = std::fs::File::options()
        .truncate(true)
        .create(true)
        .write(true)
        .open("log.txt")
        .unwrap();

    let (file, _guard_file) = tracing_appender::non_blocking(file);
    let (stderr, _guard_stderr) = tracing_appender::non_blocking(std::io::stderr());

    let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new(
        "cli=trace,hemisphere=trace,hemicore=trace,ppcjit=trace",
    ));

    let file_layer = fmt::layer().with_writer(file).with_ansi(false);
    let stderr_layer = fmt::layer().with_writer(stderr).with_ansi(true);

    let subscriber = tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .with(env_filter);

    subscriber.init();

    [_guard_file, _guard_stderr]
}

fn main() -> Result<()> {
    eyre_pretty::install()?;
    let _guard = setup_tracing();

    info!("opening panda.dol");
    let file = std::fs::File::open("panda.dol").unwrap();
    let dol = Dol::read(&mut BufReader::new(file)).unwrap();

    let mut hemisphere = Hemisphere::new(Config {
        instructions_per_block: 128,
    });

    info!("loading panda.dol");
    hemisphere.state.load(&dol);

    info!("main loop start");
    loop {
        hemisphere.exec();
        if hemisphere.state.cpu.pc == 0x8000_4010 {
            break;
        }
    }

    Ok(())
}
