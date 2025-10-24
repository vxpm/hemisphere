use crate::{Ctx, subsystem::mmio_dbg, windows::AppWindow};
use eframe::egui;
use hemisphere::{runner::State, system::gpu::command::*};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Window {
    #[serde(skip)]
    status: Status,
    #[serde(skip)]
    control: Control,
    #[serde(skip)]
    fifo: Fifo,
}

#[typetag::serde(name = "subsystem-cp")]
impl AppWindow for Window {
    fn title(&self) -> &str {
        "Command Processor"
    }

    fn prepare(&mut self, state: &mut State) {
        let core = state.core();
        let cp = &core.system.gpu.command;

        self.status = cp.status.clone();
        self.control = cp.control.clone();
        self.fifo = cp.fifo.clone();
    }

    fn show(&mut self, ui: &mut egui::Ui, _: &mut Ctx) {
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            mmio_dbg(ui, "Status", &self.status);
            mmio_dbg(ui, "Control", &self.control);
            ui.separator();

            ui.label("FIFO");
            mmio_dbg(ui, "FIFO start", &self.fifo.start);
            mmio_dbg(ui, "FIFO end", &self.fifo.end);
            mmio_dbg(ui, "FIFO high watermark", &self.fifo.high_mark);
            mmio_dbg(ui, "FIFO low watermark", &self.fifo.low_mark);
            mmio_dbg(ui, "FIFO count", &self.fifo.count);
            mmio_dbg(ui, "FIFO write ptr", &self.fifo.write_ptr);
            mmio_dbg(ui, "FIFO read ptr", &self.fifo.read_ptr);
        });
    }
}
