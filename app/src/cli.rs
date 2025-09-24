use clap::Parser;
use std::path::PathBuf;

/// Hemisphere: GameCube emulator
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to a .dol file to load and execute
    pub input: PathBuf,
    #[arg(long)]
    pub dwarf: Option<PathBuf>,
    /// Whether to start running right away
    #[arg(short, long, default_value_t = false)]
    pub run: bool,
    /// Maximum number of instructions per block
    #[arg(visible_alias("ipb"), long, default_value_t = 256)]
    pub instr_per_block: u32,
}
