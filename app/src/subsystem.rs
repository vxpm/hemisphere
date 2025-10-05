pub mod cp;

use eframe::egui;

fn mmio_dbg(ui: &mut egui::Ui, name: impl Into<egui::WidgetText>, content: &dyn std::fmt::Debug) {
    egui::CollapsingHeader::new(name).show(ui, |ui| {
        ui.small(format!("{:#?}", content));
    });
}
