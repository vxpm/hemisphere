use crate::{emulator::State, tab::Tab};
use eframe::egui::{self, CollapsingHeader};

pub struct CpuTab {}

impl Tab for CpuTab {
    fn title(&mut self) -> eframe::egui::WidgetText {
        "CPU View".into()
    }

    fn ui(&mut self, state: &mut State, ui: &mut eframe::egui::Ui) {
        CollapsingHeader::new("GPR").show(ui, |ui| {
            ui.label(format!("{}", state.emulator.pc));
        });
    }
}
