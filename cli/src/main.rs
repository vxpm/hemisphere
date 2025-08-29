mod tab;

use crate::tab::Tab;
use binrw::io::BufReader;
use clap::Parser;
use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, Hemisphere,
    dolfile::{Dol, binrw::BinRead},
    runner::Runner,
};
use ratatui::DefaultTerminal;
use std::{path::PathBuf, time::Duration};
use tracing::info;

enum Action {
    None,
    NextTab,
    PreviousTab,
    Quit,
}

pub struct App {
    runner: Runner,
    current_tab: Tab,
}

impl App {
    pub fn new() -> Self {
        Self {
            runner: Runner::new(Hemisphere::new(Config::default())),
            current_tab: Tab::Main,
        }
    }

    pub fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.current_tab.render(frame))?;
            if self.handle_events()? {
                break;
            }
        }

        Ok(())
    }

    pub fn handle_events(&mut self) -> Result<bool> {
        while crossterm::event::poll(Duration::from_millis(10))? {
            let event = crossterm::event::read()?;
            match self.current_tab.handle_event(event)? {
                Action::None => (),
                Action::NextTab => todo!(),
                Action::PreviousTab => todo!(),
                Action::Quit => return Ok(true),
            }
        }

        Ok(false)
    }
}

/// Hemisphere: GameCube emulator
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    /// Path to a .dol file to load and execute
    input: PathBuf,
}

fn setup_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    let file = std::fs::File::options()
        .truncate(true)
        .create(true)
        .write(true)
        .open("logs.txt")
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

    let mut app = App::new();
    app.runner.state().hemisphere.state.load(&dol);

    info!("starting interface");
    let terminal = ratatui::init();
    app.run(terminal)?;
    ratatui::restore();

    Ok(())
}
