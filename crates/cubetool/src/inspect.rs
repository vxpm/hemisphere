use bytesize::ByteSize;
use comfy_table::{
    Cell, CellAlignment, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use eyre_pretty::{Context, Result};
use gcwfmt::{
    apploader::Apploader,
    binrw::{BinRead, io::BufReader},
    dol,
    iso::{self, Meta},
    rvz,
};
use std::{
    io::{Cursor, Read, Seek},
    path::PathBuf,
};

use crate::vfs::{self, VfsEntryId, VfsGraph, VirtualEntry};

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

pub fn inspect_dol(input: PathBuf) -> Result<()> {
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

fn apploader_table(apploader: &Apploader) -> Result<()> {
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
        Cell::new(format!("{}", apploader.header.version)),
    ]);

    properties.add_row(vec![
        Cell::new("Entrypoint"),
        Cell::new(format!("0x{:08X}", apploader.header.entrypoint)),
    ]);

    properties.add_row(vec![
        Cell::new("Size"),
        Cell::new(format!(
            "0x{:08X} ({})",
            apploader.header.size,
            ByteSize(apploader.header.size as u64)
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Trailer Size"),
        Cell::new(format!(
            "0x{:08X} ({})",
            apploader.header.size,
            ByteSize(apploader.header.size as u64)
        )),
    ]);

    println!("{properties}");

    Ok(())
}

fn print_dir(graph: &VfsGraph, id: VfsEntryId, depth: u8, current: &str) {
    let VirtualEntry::Dir(dir) = graph.node_weight(id).unwrap() else {
        unreachable!()
    };

    let base = format!(
        "{current}{}{}",
        if current.is_empty() { "" } else { "/" },
        dir.name
    );

    let indent = |offset| {
        for _ in 0..(depth + offset) {
            print!(" ")
        }
    };

    indent(0);
    println!(
        "{}/",
        if dir.name.is_empty() && current.is_empty() {
            "root"
        } else {
            &dir.name
        }
    );

    for child in graph.neighbors(id) {
        let entry = graph.node_weight(child).unwrap();
        match entry {
            VirtualEntry::File(file) => {
                indent(2);
                println!(
                    "{} ({}/{}) ({})",
                    file.name,
                    base,
                    file.name,
                    ByteSize(file.data_length as u64)
                );
            }
            VirtualEntry::Dir(_) => {
                print_dir(graph, child, depth + 2, &base);
            }
        }
    }
}

fn inspect_iso_fs(mut iso: iso::Iso<impl Read + Seek>) -> Result<()> {
    let filesystem = vfs::VirtualFileSystem::new(&mut iso)?;
    let root = filesystem.root();
    let graph = filesystem.graph();

    print_dir(graph, root, 0, "");

    Ok(())
}

pub fn inspect_iso(input: PathBuf, filesystem: bool) -> Result<()> {
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

    if filesystem {
        println!("{info}");
        return inspect_iso_fs(iso);
    }

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
        Cell::new(format!("{}", header.meta.game_name)),
    ]);

    properties.add_row(vec![
        Cell::new("Game ID"),
        Cell::new(format!("0x{:04X}", header.meta.game_id)),
    ]);

    properties.add_row(vec![
        Cell::new("Console ID"),
        Cell::new(format!(
            "0x{:02X} ({})",
            header.meta.console_id,
            maybe(header.meta.console())
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Country Code"),
        Cell::new(format!(
            "0x{:02X} ({})",
            header.meta.country_code,
            maybe(header.meta.region())
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Game Code"),
        Cell::new(format!(
            "{} (0x{:04X})",
            header
                .meta
                .game_code_str()
                .as_deref()
                .unwrap_or("<invalid>"),
            header.meta.game_code()
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Maker Code"),
        Cell::new(format!("0x{:04X}", header.meta.maker_code)),
    ]);

    properties.add_row(vec![
        Cell::new("Disk ID"),
        Cell::new(format!("0x{:02X}", header.meta.disk_id)),
    ]);

    properties.add_row(vec![
        Cell::new("Version"),
        Cell::new(format!("0x{:02X}", header.meta.version)),
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
            header.meta.audio_streaming,
            maybe(header.meta.audio_streaming())
        )),
    ]);

    properties.add_row(vec![
        Cell::new("Stream Buffer Size"),
        Cell::new(format!(
            "0x{:02X} ({})",
            header.meta.stream_buffer_size,
            ByteSize(header.meta.stream_buffer_size as u64)
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
    }

    Ok(())
}

pub fn inspect_rvz(input: PathBuf) -> Result<()> {
    let mut file = std::fs::File::open(&input).context("opening file")?;
    let mut rvz = rvz::Rvz::new(BufReader::new(&mut file)).context("parsing .rvz file")?;

    dbg!(rvz.header());
    dbg!(rvz.disk());
    dbg!(rvz.disk_sections());

    let mut buf = vec![0; 0x25C0];
    rvz.read(0x1F800, &mut buf).unwrap();

    Ok(())
}
