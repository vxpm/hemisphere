use crate::tab::{Context, Tab};

#[derive(Default)]
pub struct LogsTab {}

impl Tab for LogsTab {
    fn title(&mut self) -> eframe::egui::WidgetText {
        "Logs".into()
    }

    fn ui(&mut self, ctx: Context, ui: &mut eframe::egui::Ui) {
        let state = ctx.state;
    }
}
