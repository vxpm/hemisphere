use crate::{colors, emulator::State, tab::Tab};
use bytesize::ByteSize;
use eframe::egui::{self, Color32, vec2};
use egui_flex::{Flex, item};
use hemisphere::jit::BlockId;

enum ModalMode {
    Instructions,
    CLIR,
    ASM,
}

#[derive(Default)]
pub struct BlocksTab {
    block_id: BlockId,
    modal_mode: Option<ModalMode>,
}

impl BlocksTab {
    // fn user_regs(&self, state: &mut State, ui: &mut eframe::egui::Ui) {}
}

impl Tab for BlocksTab {
    fn title(&mut self) -> eframe::egui::WidgetText {
        "🗖 JIT Blocks".into()
    }

    fn ui(&mut self, state: &mut State, ui: &mut eframe::egui::Ui) {
        Flex::horizontal().wrap(true).show(ui, |flex| {
            for (addr, id) in state.emulator.blocks.iter() {
                let block = state.emulator.blocks.get_by_id(id).unwrap();

                flex.add_ui(item(), |ui| {
                    egui::Frame::new()
                        .stroke(egui::Stroke::new(1.0, colors().outline_light))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(4, 4))
                        .show(ui, |ui| {
                            Flex::vertical().show(ui, |flex| {
                                flex.add_ui(item(), |ui| {
                                    ui.label(format!(
                                        "{}  ({})",
                                        addr,
                                        ByteSize(block.len() as u64)
                                    ))
                                });

                                flex.add_ui(item(), |ui| {
                                    ui.horizontal(|ui| {
                                        if ui.button("Instructions").clicked() {
                                            self.block_id = id;
                                            self.modal_mode = Some(ModalMode::Instructions);
                                        }
                                        if ui.button("CLIR").clicked() {
                                            self.block_id = id;
                                            self.modal_mode = Some(ModalMode::CLIR);
                                        }
                                        if ui.button("ASM").clicked() {
                                            self.block_id = id;
                                            self.modal_mode = Some(ModalMode::ASM);
                                        }
                                    });
                                });
                            });
                        })
                });
            }
        });

        if let Some(mode) = &self.modal_mode {
            let block = state.emulator.blocks.get_by_id(self.block_id).unwrap();
            ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);

            let modal = egui::Modal::new(egui::Id::new("blocks-modal")).show(ui.ctx(), |ui| {
                egui::Sides::new().show(
                    ui,
                    |_| {},
                    |ui| {
                        if ui.button("Close").clicked() {
                            ui.close();
                        }
                    },
                );

                let width = ui.available_width();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.set_width(width);
                    ui.set_height_range(250.0..=500.0);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    match mode {
                        ModalMode::Instructions => {
                            ui.label(block.sequence().to_string());
                        }
                        ModalMode::CLIR => {
                            ui.label(block.clir());
                        }
                        ModalMode::ASM => {
                            ui.label(block.to_string());
                        }
                    }
                });
            });

            if modal.should_close() {
                self.modal_mode = None;
            }
        }
    }
}
