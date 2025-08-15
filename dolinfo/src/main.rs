use bytesize::ByteSize;
use clap::Parser;
use comfy_table::{
    Cell, CellAlignment, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use dolfile::{Header, binrw::BinRead};
use eyre_pretty::{Context, Result};
use std::path::PathBuf;

/// CLI tool to obtain info about a .dol file.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the .dol file
    input: PathBuf,
}

fn main() -> Result<()> {
    eyre_pretty::install().unwrap();

    let config = Args::parse();

    let mut file = std::fs::File::open(&config.input).context("opening .dol file")?;
    let header = Header::read(&mut file).context("parsing .dol header")?;

    let meta = file.metadata()?;

    println!(
        " {} ({})    Entry: 0x{:08X}",
        config.input.file_name().unwrap().to_string_lossy(),
        ByteSize(meta.len()).display(),
        header.entry
    );

    let mut table = Table::new();
    table
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

    for (i, section) in header.text_sections().enumerate() {
        table.add_row(vec![
            Cell::new(format!(".text{i}")),
            Cell::new(format!("0x{:08X}", section.offset)),
            Cell::new(format!("0x{:08X}", section.target)),
            Cell::new(format!("0x{:08X}", section.size)),
            Cell::new(format!("{}", ByteSize(section.size as u64).display()))
                .set_alignment(CellAlignment::Center),
        ]);
    }

    for (i, section) in header.data_sections().enumerate() {
        table.add_row(vec![
            Cell::new(format!(".text{i}")),
            Cell::new(format!("0x{:08X}", section.offset)),
            Cell::new(format!("0x{:08X}", section.target)),
            Cell::new(format!("0x{:08X}", section.size)),
            Cell::new(format!("{}", ByteSize(section.size as u64).display()))
                .set_alignment(CellAlignment::Center),
        ]);
    }

    table.add_row(vec![
        Cell::new(".bss"),
        Cell::new("-").set_alignment(CellAlignment::Center),
        Cell::new(format!("0x{:08X}", header.bss_target)),
        Cell::new(format!("0x{:08X}", header.bss_size)),
        Cell::new(format!("{}", ByteSize(header.bss_size as u64).display()))
            .set_alignment(CellAlignment::Center),
    ]);

    println!("{table}");

    Ok(())
}
