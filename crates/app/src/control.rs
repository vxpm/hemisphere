use crate::{Ctx, State, windows::AppWindow};
use eframe::egui::{self, RichText};
use hemisphere::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default, Serialize, Deserialize)]
pub struct Window {
    #[serde(skip)]
    current_pc: u32,
    #[serde(rename(serialize = "breakpoints_to_add"), skip_deserializing)]
    breakpoints: Vec<u32>,
    #[serde(skip_serializing)]
    breakpoints_to_add: Vec<u32>,
    #[serde(skip)]
    breakpoint_to_remove: Option<u32>,
    #[serde(skip)]
    breakpoint_text: String,
    #[serde(default)]
    labels: HashMap<u32, String>,
}

impl Window {}

#[typetag::serde(name = "control")]
impl AppWindow for Window {
    fn title(&self) -> &str {
        "Control"
    }

    fn prepare(&mut self, state: &mut State) {
        for breakpoint in self.breakpoints_to_add.drain(..) {
            state.add_breakpoint(Address(breakpoint));
        }

        if let Some(breakpoint) = self.breakpoint_to_remove.take() {
            state.remove_breakpoint(Address(breakpoint));
        }

        self.breakpoints.clear();
        self.breakpoints
            .extend(state.breakpoints.iter().map(|b| b.value()));
        self.labels.retain(|b, _| self.breakpoints.contains(b));

        self.current_pc = state.emulator.system.cpu.pc.value();
    }

    fn show(&mut self, ui: &mut egui::Ui, ctx: &mut Ctx) {
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
                    self.breakpoints_to_add.push(addr);
                }
            }
        });

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for breakpoint in &self.breakpoints {
                    ui.horizontal(|ui| {
                        if ui.button("🗑").clicked() {
                            self.breakpoint_to_remove = Some(*breakpoint);
                        }

                        let text = RichText::new(Address(*breakpoint).to_string()).color(
                            if *breakpoint == self.current_pc {
                                egui::Color32::LIGHT_RED
                            } else {
                                egui::Color32::GRAY
                            },
                        );

                        ui.label(text);
                    });

                    let label = self.labels.entry(*breakpoint).or_default();
                    ui.text_edit_singleline(label);
                }
            });
    }
}
