use clap::Parser;
use std::path::PathBuf;

/// Hemisphere: GameCube emulator
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to the ROM to load and execute.
    ///
    /// Supported format is .iso. To sideload executables, use the `exec` argument.
    #[arg(short, long)]
    pub iso: Option<PathBuf>,
    /// Path to the executable to sideload and execute.
    ///
    /// Supported format is .dol.
    #[arg(long)]
    pub exec: Option<PathBuf>,
    /// Path to the IPL ROM.
    #[arg(long)]
    pub ipl: Option<PathBuf>,
    /// Path to a file to use as a debug info provider.
    #[arg(long)]
    pub debug: Option<PathBuf>,
    /// Whether to start running right away
    #[arg(short, long, default_value_t = false)]
    pub run: bool,
    /// Maximum number of instructions per block
    #[arg(visible_alias("ipb"), long, default_value_t = 256)]
    pub instr_per_block: u32,
}
