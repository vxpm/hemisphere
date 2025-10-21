use binrw::{BinRead, BinWrite, io::BufReader};
use bytesize::ByteSize;
use clap::{Parser, Subcommand};
use comfy_table::{
    Cell, CellAlignment, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use eyre_pretty::{Context, ContextCompat, Result, bail};
use std::{io::BufWriter, path::PathBuf};

#[derive(Debug, Subcommand)]
enum Command {
    Inspect {
        /// Path to the input file
        input: PathBuf,
    },
    Convert {
        /// Path to the input file
        input: PathBuf,
        /// Path to the output file
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

fn dol_table(header: &dol::Header) -> Result<()> {
    let mut sections = Table::new();
    sections
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Section").set_alignment(CellAlignment::Center),
            Cell::new("Offset").set_alignment(CellAlignment::Center),
            Cell::new("Target").set_alignment(CellAlignment::Center),
            Cell::new("Length").set_alignment(CellAlignment::Center),
            Cell::new("Length (Bytes)").set_alignment(CellAlignment::Center),
        ]);

    let mut row = |name, section: dol::SectionInfo| {
        sections.add_row(vec![
            Cell::new(name),
            Cell::new(format!("0x{:08X}", section.offset)),
            Cell::new(format!("0x{:08X}", section.target)),
            Cell::new(format!("0x{:08X}", section.size)),
            Cell::new(format!("{}", ByteSize(section.size as u64).display()))
                .set_alignment(CellAlignment::Center),
        ]);
    };

    for (i, section) in header.text_sections().enumerate() {
        row(format!(".text{i}"), section)
    }

    for (i, section) in header.data_sections().enumerate() {
        row(format!(".data{i}"), section)
    }

    if header.bss_size != 0 {
        sections.add_row(vec![
            Cell::new(".bss"),
            Cell::new("-").set_alignment(CellAlignment::Center),
            Cell::new(format!("0x{:08X}", header.bss_target)),
            Cell::new(format!("0x{:08X}", header.bss_size)),
            Cell::new(format!("{}", ByteSize(header.bss_size as u64).display()))
                .set_alignment(CellAlignment::Center),
        ]);
    }

    println!("{sections}");

    Ok(())
}

fn inspect_dol(input: PathBuf) -> Result<()> {
    let mut file = std::fs::File::open(&input).context("opening file")?;
    let header = dol::Header::read(&mut file).context("parsing .dol header")?;
    let meta = file.metadata()?;

    let mut info = Table::new();
    info.load_preset(comfy_table::presets::NOTHING)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new(format!(
                "{} ({})",
                input.file_name().unwrap().to_string_lossy(),
                ByteSize(meta.len()).display()
            )),
            Cell::new(format!("Entry: 0x{:08X}", header.entry)),
        ]);

    println!("{info}");
    dol_table(&header)?;

    Ok(())
}

fn apploader_table(apploader: &iso::Apploader) -> Result<()> {
    let mut properties = Table::new();
    properties
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Property").set_alignment(CellAlignment::Center),
            Cell::new("Value").set_alignment(CellAlignment::Center),
        ]);

    properties.add_row(vec![
        Cell::new("Version"),
        Cell::new(format!("{}", apploader.version)),
    ]);

    properties.add_row(vec![
        Cell::new("Entrypoint"),
        Cell::new(format!("0x{:08X}", apploader.entrypoint)),
    ]);

    properties.add_row(vec![
        Cell::new("Size"),
        Cell::new(format!(
            "0x{:08X} ({})",
            apploader.size,
            ByteSize(apploader.size as u64)
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Trailer Size"),
        Cell::new(format!(
            "0x{:08X} ({})",
            apploader.size,
            ByteSize(apploader.size as u64)
        )),
    ]);

    println!("{properties}");

    Ok(())
}

fn inspect_iso(input: PathBuf) -> Result<()> {
    let mut file = std::fs::File::open(&input).context("opening file")?;
    let meta = file.metadata()?;
    let mut iso = iso::Iso::new(BufReader::new(&mut file)).context("parsing .iso header")?;

    let mut info = Table::new();
    info.load_preset(comfy_table::presets::NOTHING)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![Cell::new(format!(
            "{} ({})",
            input.file_name().unwrap().to_string_lossy(),
            ByteSize(meta.len()).display()
        ))]);

    let mut properties = Table::new();
    properties
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Property").set_alignment(CellAlignment::Center),
            Cell::new("Value").set_alignment(CellAlignment::Center),
        ]);

    fn maybe(value: Option<impl std::fmt::Debug>) -> String {
        value
            .map(|x| format!("{x:?}"))
            .unwrap_or("<unknown>".to_owned())
    }

    let header = iso.header();
    properties.add_row(vec![
        Cell::new("Game Name"),
        Cell::new(format!("{}", header.game_name)),
    ]);

    properties.add_row(vec![
        Cell::new("Game ID"),
        Cell::new(format!("0x{:04X}", header.game_id)),
    ]);

    properties.add_row(vec![
        Cell::new("Console ID"),
        Cell::new(format!(
            "0x{:02X} ({})",
            header.console_id,
            maybe(header.console())
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Country Code"),
        Cell::new(format!(
            "0x{:02X} ({})",
            header.country_code,
            maybe(header.country())
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Game Code"),
        Cell::new(format!(
            "{} (0x{:04X})",
            header.game_code_str().as_deref().unwrap_or("<invalid>"),
            header.game_code()
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Maker Code"),
        Cell::new(format!("0x{:04X}", header.maker_code)),
    ]);

    properties.add_row(vec![
        Cell::new("Disk ID"),
        Cell::new(format!("0x{:02X}", header.disk_id)),
    ]);

    properties.add_row(vec![
        Cell::new("Version"),
        Cell::new(format!("0x{:02X}", header.version)),
    ]);

    properties.add_row(vec![
        Cell::new("Bootfile Offset"),
        Cell::new(format!("0x{:08X}", header.bootfile_offset)),
    ]);

    properties.add_row(vec![
        Cell::new("Debug Monitor Offset"),
        Cell::new(format!("0x{:08X}", header.debug_monitor_offset)),
    ]);

    properties.add_row(vec![
        Cell::new("Debug Monitor Target"),
        Cell::new(format!("0x{:08X}", header.debug_monitor_target)),
    ]);

    properties.add_row(vec![
        Cell::new("Filesystem Offset"),
        Cell::new(format!("0x{:08X}", header.filesystem_offset)),
    ]);

    properties.add_row(vec![
        Cell::new("Filesystem Size"),
        Cell::new(format!(
            "0x{:08X} ({})",
            header.filesystem_size,
            ByteSize(header.filesystem_size as u64)
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Max. Filesystem Size"),
        Cell::new(format!(
            "0x{:08X} ({})",
            header.max_filesystem_size,
            ByteSize(header.max_filesystem_size as u64)
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Audio Streaming"),
        Cell::new(format!(
            "0x{:02X} ({})",
            header.audio_streaming,
            maybe(header.audio_streaming())
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Stream Buffer Size"),
        Cell::new(format!(
            "0x{:02X} ({})",
            header.stream_buffer_size,
            ByteSize(header.stream_buffer_size as u64)
        )),
    ]);

    properties.add_row(vec![
        Cell::new("User Position"),
        Cell::new(format!("0x{:08X}", header.user_position)),
    ]);

    properties.add_row(vec![
        Cell::new("User Length"),
        Cell::new(format!(
            "0x{:08X} ({})",
            header.user_length,
            ByteSize(header.user_length as u64)
        )),
    ]);

    println!("{info}");
    println!("{properties}");

    if let Ok(apploader) = iso.apploader() {
        let mut info = Table::new();
        info.load_preset(comfy_table::presets::NOTHING)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![Cell::new(format!("Apploader"))]);

        println!("{info}");
        apploader_table(&apploader)?;
    }

    if let Ok(bootfile) = iso.bootfile() {
        let mut info = Table::new();
        info.load_preset(comfy_table::presets::NOTHING)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new(format!("Bootfile (.dol)")),
                Cell::new(format!("Entry: 0x{:08X}", bootfile.header.entry)),
            ]);

        println!("{info}");
        dol_table(&bootfile.header)?;

        // if let Some(path) = config.bootfile {
        //     bootfile
        //         .write(&mut std::fs::File::create(path).unwrap())
        //         .unwrap();
        // }
    }

    Ok(())
}

fn convert_elf_to_dol(input: PathBuf, output: PathBuf) -> Result<()> {
    let input = std::fs::File::open(&input).context("opening input file")?;
    let dol = dol::elf_to_dol(BufReader::new(input))?;

    let mut output = BufWriter::new(std::fs::File::create(&output).context("opening output file")?);
    dol.write(&mut output)?;

    Ok(())
}

fn main() -> Result<()> {
    eyre_pretty::install().unwrap();

    let config = Args::parse();
    match config.command {
        Command::Inspect { input } => {
            let extension = input
                .extension()
                .and_then(|ext| ext.to_str())
                .context("unknown or missing file extension")?;

            match extension {
                "dol" => inspect_dol(input),
                "iso" => inspect_iso(input),
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
    }
}
