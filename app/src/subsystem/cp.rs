use crate::{Ctx, WindowUi, subsystem::mmio_dbg};
use eframe::egui;
use hemisphere::runner::State;

#[derive(Default)]
pub struct Window {}

impl Window {}

impl WindowUi for Window {
    fn title(&self) -> &str {
        "Command Processor"
    }

    fn show(&mut self, ui: &mut egui::Ui, _: &mut Ctx, state: &mut State) {
        let core = state.core();
        let cp = &core.system.bus.gpu.command;

        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            mmio_dbg(ui, "Status", &cp.status);
            mmio_dbg(ui, "Control", &cp.control);
            ui.separator();

            ui.label("FIFO");
            mmio_dbg(ui, "FIFO start", &cp.fifo_start);
            mmio_dbg(ui, "FIFO end", &cp.fifo_end);
            mmio_dbg(ui, "FIFO high watermark", &cp.fifo_high_mark);
            mmio_dbg(ui, "FIFO low mark", &cp.fifo_low_mark);
            mmio_dbg(ui, "FIFO count", &cp.fifo_count);
            mmio_dbg(ui, "FIFO write ptr", &cp.fifo_write_ptr);
            mmio_dbg(ui, "FIFO read ptr", &cp.fifo_read_ptr);
        });
    }
}
