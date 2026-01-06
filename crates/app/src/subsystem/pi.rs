use crate::{AppWindow, Ctx, State, subsystem::mmio_dbg};
use eframe::egui;
use hemisphere::Address;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Window {
    #[serde(skip)]
    fifo_start: Address,
    #[serde(skip)]
    fifo_end: Address,
    #[serde(skip)]
    fifo_current: Address,
}

#[typetag::serde(name = "subsystem-pi")]
impl AppWindow for Window {
    fn title(&self) -> &str {
        "Processor Interface"
    }

    fn prepare(&mut self, state: &mut State) {
        let core = &state.emulator;
        let pi = &core.system.processor;
        self.fifo_start = pi.fifo_start;
        self.fifo_end = pi.fifo_end;
        self.fifo_current = pi.fifo_current.address();
    }

    fn show(&mut self, ui: &mut egui::Ui, _: &mut Ctx) {
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            mmio_dbg(ui, "FIFO start", &self.fifo_start);
            mmio_dbg(ui, "FIFO end", &self.fifo_end);
            mmio_dbg(ui, "FIFO current", &self.fifo_current);
        });
    }
}
