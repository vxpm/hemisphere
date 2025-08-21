use crate::tab::Tab;

pub struct ControlTab {}

impl Tab for ControlTab {
    fn title(&mut self) -> eframe::egui::WidgetText {
        "Control".into()
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui) {
        ui.label("hi i'm a label");
    }
}
