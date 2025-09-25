use crate::WindowUi;
use eframe::egui::{self, Color32};
use egui_extras::{Column, TableBuilder};
use hemisphere::{
    Address,
    arch::disasm::{Extensions, Ins, ParsedIns},
    runner::State,
};

pub struct Window {
    target: Address,
    target_text: String,
    follow_pc: bool,
    simplified: bool,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            target: Default::default(),
            target_text: String::new(),
            follow_pc: true,
            simplified: true,
        }
    }
}

impl Window {}

impl WindowUi for Window {
    fn title(&self) -> &str {
        "📼 Disassembly"
    }

    fn show(&mut self, ui: &mut eframe::egui::Ui, state: &mut State) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.follow_pc, "Follow PC");
            ui.checkbox(&mut self.simplified, "Simplified");
        });

        if !self.follow_pc {
            ui.horizontal(|ui| {
                ui.label("Target: ");
                if ui.text_edit_singleline(&mut self.target_text).lost_focus() {
                    let clean = self.target_text.trim_prefix("0x").replace("_", "");
                    if let Ok(addr) = u32::from_str_radix(&clean, 16) {
                        self.target = Address(addr);
                    }
                }
            });
        }

        let builder = TableBuilder::new(ui)
            .auto_shrink(true)
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::remainder());

        let table = builder.header(20.0, |mut header| {
            header.col(|ui| {
                ui.label("Address");
            });
            header.col(|ui| {
                ui.label("Instruction");
            });
        });

        table.body(|mut body| {
            let hemi = state.hemisphere();
            if self.follow_pc {
                self.target = hemi.system.cpu.pc;
            }

            let ui = body.ui_mut();
            ui.spacing_mut().item_spacing = egui::Vec2::new(5.0, 0.0);
            ui.set_max_width(ui.available_width());

            let rows = (body.ui_mut().available_height() / 20.0) as u32;

            let mut current = self.target - 4 * (rows / 2);
            for _ in 0..rows {
                body.row(20.0, |mut row| {
                    row.col(|ui| {
                        let color = if current == hemi.system.cpu.pc {
                            Color32::LIGHT_BLUE
                        } else if current == self.target {
                            Color32::LIGHT_GREEN
                        } else {
                            Color32::GRAY
                        };

                        let text = egui::RichText::new(current.to_string())
                            .family(egui::FontFamily::Monospace)
                            .color(color);

                        ui.label(text);
                    });

                    row.col(|ui| {
                        let translated = hemi
                            .system
                            .translate_instr_addr(current)
                            .unwrap_or_default();
                        let code = hemi.system.read_pure(translated).unwrap_or(0);
                        let ins = Ins::new(code, Extensions::gekko_broadway());

                        let mut parsed = ParsedIns::new();
                        if self.simplified {
                            ins.parse_simplified(&mut parsed);
                        } else {
                            ins.parse_basic(&mut parsed);
                        }

                        ui.label(parsed.to_string());
                    });
                });

                current += 4;
            }
        });
    }
}
