use crate::{Ctx, windows::AppWindow};
use eframe::egui::{self, Color32};
use egui_extras::{Column, TableBuilder};
use hemisphere::{runner::State, system::eabi::CallStack};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Window {
    #[serde(skip)]
    call_stack: CallStack,
}

#[typetag::serde(name = "call_stack")]
impl AppWindow for Window {
    fn title(&self) -> &str {
        "Call Stack"
    }

    fn prepare(&mut self, state: &mut State) {
        self.call_stack = state.core().system.call_stack();
    }

    fn show(&mut self, ui: &mut egui::Ui, _: &mut Ctx) {
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
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
