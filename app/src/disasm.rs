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
    follow_pc: bool,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            target: Default::default(),
            follow_pc: true,
        }
    }
}

impl Window {}

impl WindowUi for Window {
    fn title(&self) -> &str {
        "📼 Disassembly"
    }

    fn show(&mut self, ui: &mut eframe::egui::Ui, state: &mut State) {
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
                        ins.parse_simplified(&mut parsed);

                        ui.label(parsed.to_string());
                    });
                });

                current += 4;
            }
        });
    }
}
