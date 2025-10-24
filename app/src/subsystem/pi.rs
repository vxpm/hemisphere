use crate::{Ctx, WindowUi, subsystem::mmio_dbg};
use eframe::egui;
use hemisphere::runner::State;

#[derive(Default)]
pub struct Window;

impl WindowUi for Window {
    fn title(&self) -> &str {
        "Processor Interface"
    }

    fn show(&mut self, ui: &mut egui::Ui, _: &mut Ctx, state: &mut State) {
        let core = state.core();
        let pi = &core.system.processor;

        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            mmio_dbg(ui, "FIFO start", &pi.fifo_start);
            mmio_dbg(ui, "FIFO end", &pi.fifo_end);
            mmio_dbg(ui, "FIFO current", &pi.fifo_current);
        });
    }
}
