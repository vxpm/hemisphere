mod inspect;
mod vfs;

use binrw::{BinWrite, io::BufReader};
use clap::{Parser, Subcommand};
use eyre_pretty::{Context, ContextCompat, Result, bail, eyre};
use std::{
    io::{BufWriter, Read, Seek, SeekFrom},
    path::PathBuf,
};

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
        /// Target to extract
        #[arg(short, long)]
        target: String,
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

fn extract_iso_file(input: PathBuf, output: PathBuf, target: String) -> Result<()> {
    let input = std::fs::File::open(&input).context("opening input file")?;
    let mut iso = iso::Iso::new(BufReader::new(input))?;
    let filesystem = vfs::VirtualFileSystem::new(&mut iso)?;

    let target = filesystem
        .path_to_entry(target)
        .ok_or(eyre!("no entry with such path in the filesystem"))?;

    let entry = filesystem.graph().node_weight(target).unwrap();
    let vfs::VirtualEntry::File(file) = entry else {
        bail!("entry at given path is a directory");
    };

    let mut output = BufWriter::new(std::fs::File::create(&output).context("opening output file")?);
    iso.reader()
        .seek(SeekFrom::Start(file.data_offset as u64))?;

    let mut reader = iso.reader().take(file.data_length as u64);
    std::io::copy(&mut reader, &mut output)?;

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
            target,
            input,
            output,
        } => {
            let extension = input
                .extension()
                .and_then(|ext| ext.to_str())
                .context("unknown or missing file extension")?;

            match (extension, &*target) {
                ("iso", "bootfile") => extract_bootfile(input, output),
                ("iso", _) => extract_iso_file(input, output, target),
                _ => bail!("unsupported extension/target combination"),
            }
        }
    }
}
