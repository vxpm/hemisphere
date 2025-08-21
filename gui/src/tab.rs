pub mod control;

use eframe::egui;

pub trait Tab {
    fn title(&mut self) -> egui::WidgetText;
    fn ui(&mut self, ui: &mut egui::Ui);

    fn is_closeable(&self) -> bool {
        true
    }
}
