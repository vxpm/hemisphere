use std::{borrow::Cow, path::Path};

use crate::{Ctx, State, windows::AppWindow};
use addr2line::gimli;
use eframe::egui::{self, Color32};
use egui_extras::{Column, TableBuilder};
use hemisphere::{
    Address,
    system::{
        eabi::CallStack,
        executable::{DebugInfo, Location},
    },
};
use mapfile_parser::MapFile;
use serde::{Deserialize, Serialize};

pub struct Addr2LineDebug(pub addr2line::Loader);

impl DebugInfo for Addr2LineDebug {
    fn find_symbol(&self, addr: Address) -> Option<String> {
        self.0
            .find_symbol(addr.value() as u64)
            .map(|s| addr2line::demangle_auto(Cow::Borrowed(s), Some(gimli::DW_LANG_C_plus_plus)))
            .map(|s| s.into_owned())
    }

    fn find_location(&self, addr: Address) -> Option<Location<'_>> {
        self.0
            .find_location(addr.value() as u64)
            .ok()
            .flatten()
            .map(|l| Location {
                file: l.file.map(Cow::Borrowed),
                line: l.line,
                column: l.column,
            })
    }
}

pub struct MapFileDebug(MapFile);

impl MapFileDebug {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self(MapFile::new_from_map_file(path.as_ref()))
    }
}

impl DebugInfo for MapFileDebug {
    fn find_symbol(&self, addr: Address) -> Option<String> {
        self.0
            .find_symbol_by_vram(addr.0 as u64)
            .0
            .map(|s| addr2line::demangle_auto(Cow::Borrowed(&s.symbol.name), None).into_owned())
    }

    fn find_location(&self, addr: Address) -> Option<Location<'_>> {
        self.0
            .find_symbol_by_vram(addr.0 as u64)
            .0
            .map(|s| Location {
                file: Some(s.section.filepath.to_string_lossy()),
                line: None,
                column: None,
            })
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Window {
    #[serde(skip)]
    location: Option<Location<'static>>,
    #[serde(skip)]
    call_stack: CallStack,
}

#[typetag::serde(name = "call_stack")]
impl AppWindow for Window {
    fn title(&self) -> &str {
        "Call Stack"
    }

    fn prepare(&mut self, state: &mut State) {
        let emulator = &state.emulator;
        self.call_stack = emulator.system.call_stack();
        self.location = emulator.system.config.debug_info.as_ref().and_then(|d| {
            d.find_location(emulator.system.cpu.pc)
                .map(|l| l.into_owned())
        });
    }

    fn show(&mut self, ui: &mut egui::Ui, _: &mut Ctx) {
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(location) = &self.location {
                    ui.label(format!(
                        "Location of PC: {}:{}:{}",
                        location.file.as_deref().unwrap_or("<unknown>"),
                        location.line.unwrap_or(0),
                        location.column.unwrap_or(0)
                    ));
                } else {
                    ui.label("Location of PC: <unknown>");
                }
            });

            ui.separator();

            let builder = TableBuilder::new(ui)
                .auto_shrink(egui::Vec2b::new(false, true))
                .striped(true)
                .resizable(false)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::auto()) // addr
                .column(Column::auto()) // stack
                .column(Column::remainder().at_least(200.0)); // symbol

            let table = builder.header(20.0, |mut header| {
                header.col(|ui| {
                    ui.label("Address");
                });
                header.col(|ui| {
                    ui.label("Stack");
                });
                header.col(|ui| {
                    ui.label("Symbol");
                });
            });

            table.body(|mut body| {
                for call in &self.call_stack.0 {
                    body.row(20.0, |mut row| {
                        row.col(|ui| {
                            let text = egui::RichText::new(call.address.to_string())
                                .family(egui::FontFamily::Monospace)
                                .color(Color32::LIGHT_BLUE);

                            ui.label(text);
                        });

                        row.col(|ui| {
                            let text = egui::RichText::new(call.stack.to_string())
                                .family(egui::FontFamily::Monospace)
                                .color(Color32::LIGHT_GREEN);

                            ui.label(text);
                        });

                        row.col(|ui| {
                            let text =
                                egui::RichText::new(call.symbol.as_deref().unwrap_or("<unknown>"))
                                    .family(egui::FontFamily::Monospace)
                                    .color(Color32::GRAY);

                            ui.label(text);
                        });
                    })
                }
            });
        });
    }
}
