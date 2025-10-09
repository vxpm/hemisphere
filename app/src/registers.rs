use crate::{Ctx, WindowUi};
use eframe::egui::{self, Color32};
use egui_extras::{Column, TableBuilder};
use hemisphere::{Address, runner::State};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Group {
    #[default]
    GPR,
    FPR,
}

#[derive(Default)]
pub struct Window {
    group: Group,
}

impl Window {
    fn gpr(&self, ui: &mut egui::Ui, state: &mut State) {
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
            for (gpr, value) in state.core().system.cpu.user.gpr.iter().copied().enumerate() {
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

    fn fpr(&self, ui: &mut egui::Ui, state: &mut State) {
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
            for (fpr, value) in state.core().system.cpu.user.fpr.iter().copied().enumerate() {
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

impl WindowUi for Window {
    fn title(&self) -> &str {
        "Registers"
    }

    fn show(&mut self, ui: &mut egui::Ui, _: &mut Ctx, state: &mut State) {
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            egui::ComboBox::from_label("Group")
                .selected_text(format!("{:?}", self.group))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.group, Group::GPR, "GPR");
                    ui.selectable_value(&mut self.group, Group::FPR, "FPR");
                });

            ui.separator();

            match self.group {
                Group::GPR => self.gpr(ui, state),
                Group::FPR => self.fpr(ui, state),
            }
        });
    }
}
