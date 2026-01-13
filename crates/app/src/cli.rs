use clap::{Args, Parser};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct PpcjitConfig {
    /// Maximum number of instructions per block
    #[arg(visible_alias("ipb"), long, default_value_t = 128)]
    pub instr_per_block: u32,
    /// Whether to treat syscalls as no-ops
    #[arg(long, default_value_t = false)]
    pub nop_syscalls: bool,
    /// Whether to ignore the FPU enabled bit in MSR
    #[arg(long, default_value_t = false)]
    pub force_fpu: bool,
    /// Whether to ignore unimplemented instructions
    #[arg(long, default_value_t = false)]
    pub ignore_unimplemented_inst: bool,
    /// Whether to clear the JIT block cache
    #[arg(long, default_value_t = false)]
    pub clear_cache: bool,
}

/// Lazuli: GameCube emulator
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Config {
    #[command(flatten)]
    pub ppcjit: PpcjitConfig,
    /// Path to the IPL ROM.
    #[arg(long)]
    pub ipl: Option<PathBuf>,
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
    /// Whether to load IPL instead of HLEing it for loading games.
    #[arg(long, default_value_t = false)]
    pub force_ipl: bool,
    /// Path to a file to use as a debug info provider.
    #[arg(long)]
    pub debug: Option<PathBuf>,
    /// Whether to start running right away
    #[arg(short, long, default_value_t = false)]
    pub run: bool,
}
