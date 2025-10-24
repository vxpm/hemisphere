use crate::{Ctx, windows::AppWindow};
use eframe::egui::{self, Color32};
use egui_extras::{Column, TableBuilder};
use hemisphere::{arch::Cpu, runner::State};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
enum Group {
    #[default]
    Gpr,
    Fpr,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Window {
    group: Group,
    #[serde(skip)]
    cpu: Cpu,
}

impl Window {
    fn gpr(&self, ui: &mut egui::Ui) {
        let builder = TableBuilder::new(ui)
            .auto_shrink(egui::Vec2b::new(false, true))
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::remainder());

        let table = builder.header(20.0, |mut header| {
            header.col(|ui| {
                ui.label("GPR");
            });
            header.col(|ui| {
                ui.label("Hex");
            });
        });

        table.body(|mut body| {
            for (gpr, value) in self.cpu.user.gpr.iter().copied().enumerate() {
                body.row(20.0, |mut row| {
                    row.col(|ui| {
                        let text = egui::RichText::new(format!("R{gpr:02}"))
                            .family(egui::FontFamily::Monospace)
                            .color(Color32::LIGHT_BLUE);

                        ui.label(text);
                    });

                    row.col(|ui| {
                        let text = egui::RichText::new(format!("{value:08X}"))
                            .family(egui::FontFamily::Monospace)
                            .color(Color32::LIGHT_GREEN);

                        ui.label(text);
                    });
                })
            }
        });
    }

    fn fpr(&self, ui: &mut egui::Ui) {
        let builder = TableBuilder::new(ui)
            .auto_shrink(egui::Vec2b::new(false, true))
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::remainder().at_least(100.0))
            .column(Column::remainder().at_least(100.0));

        let table = builder.header(20.0, |mut header| {
            header.col(|ui| {
                ui.label("FPR");
            });
            header.col(|ui| {
                ui.label("PS0");
            });
            header.col(|ui| {
                ui.label("PS1");
            });
        });

        table.body(|mut body| {
            for (fpr, value) in self.cpu.user.fpr.iter().copied().enumerate() {
                body.row(20.0, |mut row| {
                    row.col(|ui| {
                        let text = egui::RichText::new(format!("F{fpr:02}"))
                            .family(egui::FontFamily::Monospace)
                            .color(Color32::LIGHT_BLUE);

                        ui.label(text);
                    });

                    row.col(|ui| {
                        let text = egui::RichText::new(format!("{}", value.0[0]))
                            .family(egui::FontFamily::Monospace)
                            .color(Color32::LIGHT_GREEN);

                        ui.label(text);
                    });

                    row.col(|ui| {
                        let text = egui::RichText::new(format!("{}", value.0[1]))
                            .family(egui::FontFamily::Monospace)
                            .color(Color32::LIGHT_GREEN);

                        ui.label(text);
                    });
                })
            }
        });
    }
}

#[typetag::serde(name = "registers")]
impl AppWindow for Window {
    fn title(&self) -> &str {
        "Registers"
    }

    fn prepare(&mut self, state: &mut State) {
        self.cpu = state.core().system.cpu.clone();
    }

    fn show(&mut self, ui: &mut egui::Ui, _: &mut Ctx) {
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            egui::ComboBox::from_label("Group")
                .selected_text(format!("{:?}", self.group))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.group, Group::Gpr, "GPR");
                    ui.selectable_value(&mut self.group, Group::Fpr, "FPR");
                });

            ui.separator();

            match self.group {
                Group::Gpr => self.gpr(ui),
                Group::Fpr => self.fpr(ui),
            }
        });
    }
}
