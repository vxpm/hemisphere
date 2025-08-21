use crate::{emulator::State, tab::Tab};
use eframe::egui::{self, Color32};
use egui_flex::{Flex, item};

pub struct BlocksTab {}

impl BlocksTab {
    // fn user_regs(&self, state: &mut State, ui: &mut eframe::egui::Ui) {}
}

impl Tab for BlocksTab {
    fn title(&mut self) -> eframe::egui::WidgetText {
        "JIT Blocks".into()
    }

    fn ui(&mut self, state: &mut State, ui: &mut eframe::egui::Ui) {
        Flex::horizontal().wrap(true).show(ui, |flex| {
            for block in state.emulator.blocks.iter() {
                flex.add_ui(item(), |ui| {
                    egui::Frame::new()
                        .stroke(egui::Stroke::new(1.0, Color32::WHITE))
                        .inner_margin(egui::Margin::symmetric(4, 4))
                        .show(ui, |ui| {
                            ui.label(format!("{}", block.0));
                        })
                });
            }
        });
        // });
    }
}
