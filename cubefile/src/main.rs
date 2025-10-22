mod inspect;

use binrw::{BinWrite, io::BufReader};
use clap::{Parser, Subcommand, ValueEnum};
use eyre_pretty::{Context, ContextCompat, Result, bail};
use std::{io::BufWriter, path::PathBuf};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExtractKind {
    Bootfile,
}

#[derive(Debug, Subcommand)]
enum Command {
    Inspect {
        /// Path to the input file
        #[arg(short, long)]
        input: PathBuf,
        /// Whether to inspect the filesystem.
        #[arg(long, default_value_t = false)]
        filesystem: bool,
    },
    Convert {
        /// Path to the input file
        #[arg(short, long)]
        input: PathBuf,
        /// Path to the output file
        #[arg(short, long)]
        output: PathBuf,
    },
    Extract {
        /// What to extract from the input
        kind: ExtractKind,
        /// Path to the input file
        #[arg(short, long)]
        input: PathBuf,
        /// Path to the output file
        #[arg(short, long)]
        output: PathBuf,
    },
}

/// A CLI to inspect and manipulate files related to the GameCube.
///
/// Supported formats: .dol, .iso, .elf.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Action to take
    #[command(subcommand)]
    command: Command,
}

fn convert_elf_to_dol(input: PathBuf, output: PathBuf) -> Result<()> {
    let input = std::fs::File::open(&input).context("opening input file")?;
    let dol = dol::elf_to_dol(BufReader::new(input))?;

    let mut output = BufWriter::new(std::fs::File::create(&output).context("opening output file")?);
    dol.write(&mut output)?;

    Ok(())
}

fn extract_bootfile(input: PathBuf, output: PathBuf) -> Result<()> {
    let input = std::fs::File::open(&input).context("opening input file")?;
    let mut iso = iso::Iso::new(BufReader::new(input))?;

    let mut output = BufWriter::new(std::fs::File::create(&output).context("opening output file")?);
    let dol = iso.bootfile()?;
    dol.write(&mut output)?;

    Ok(())
}

fn main() -> Result<()> {
    eyre_pretty::install().unwrap();

    let config = Args::parse();
    match config.command {
        Command::Inspect { input, filesystem } => {
            let extension = input
                .extension()
                .and_then(|ext| ext.to_str())
                .context("unknown or missing file extension")?;

            match extension {
                "dol" => inspect::inspect_dol(input),
                "iso" => inspect::inspect_iso(input, filesystem),
                _ => bail!("unknown or missing file extension"),
            }
        }
        Command::Convert { input, output } => {
            let extension = input
                .extension()
                .and_then(|ext| ext.to_str())
                .context("unknown or missing file extension")?;

            match extension {
                "elf" => convert_elf_to_dol(input, output),
                _ => bail!("unknown or missing file extension"),
            }
        }
        Command::Extract {
            kind,
            input,
            output,
        } => {
            let extension = input
                .extension()
                .and_then(|ext| ext.to_str())
                .context("unknown or missing file extension")?;

            match (extension, kind) {
                ("iso", ExtractKind::Bootfile) => extract_bootfile(input, output),
                _ => bail!("unsupported extension/target combination"),
            }
        }
    }
}
