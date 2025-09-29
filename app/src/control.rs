use crate::{Ctx, WindowUi};
use eframe::egui;
use hemisphere::{Address, runner::State};

#[derive(Default)]
pub struct Window {
    breakpoint_text: String,
}

impl Window {}

impl WindowUi for Window {
    fn title(&self) -> &str {
        "🖮 Control"
    }

    fn show(&mut self, ui: &mut egui::Ui, ctx: &mut Ctx, state: &mut State) {
        ui.set_max_width(150.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut ctx.running, "Run");

            let button = egui::Button::new("Step");
            if ui.add_enabled(!ctx.running, button).clicked() {
                ctx.step = true;
            }
        });

        ui.separator();
        ui.label("Breakpoints");

        ui.horizontal(|ui| {
            ui.scope(|ui| {
                ui.set_max_width(100.0);
                ui.text_edit_singleline(&mut self.breakpoint_text);
            });

            if ui.button("Add").clicked() {
                let clean = self.breakpoint_text.trim_prefix("0x").replace("_", "");
                if let Ok(addr) = u32::from_str_radix(&clean, 16) {
                    let addr = Address(addr);
                    state.add_breakpoint(addr);
                }
            }
        });

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let mut remove = None;
                for breakpoint in state.breakpoints() {
                    ui.horizontal(|ui| {
                        if ui.button("🗑").clicked() {
                            remove = Some(*breakpoint);
                        }

                        ui.label(breakpoint.to_string());
                    });
                }

                if let Some(remove) = remove {
                    state.remove_breakpoint(remove);
                }
            });
    }
}
