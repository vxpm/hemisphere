use crate::{
    colors,
    tab::{Context, Tab},
};
use bytesize::ByteSize;
use eframe::egui::{self, vec2};
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

    fn ui(&mut self, ctx: Context, ui: &mut eframe::egui::Ui) {
        let state = ctx.state;
        Flex::horizontal().wrap(true).show(ui, |flex| {
            for (addr, id) in state.emulator.blocks.iter() {
                let block = state.emulator.blocks.get_by_id(id).unwrap();

                flex.add_ui(item(), |ui| {
                    egui::Frame::new()
                        .stroke(egui::Stroke::new(1.0, colors().outline_light))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(4, 4))
                        .show(ui, |ui| {
                            Flex::vertical()
                                .align_items(egui_flex::FlexAlign::Center)
                                .show(ui, |flex| {
                                    let title =
                                        egui::RichText::new(format!("{}", addr)).monospace();
                                    flex.add(item(), egui::Label::new(title));

                                    flex.add(
                                        item(),
                                        egui::Label::new(format!(
                                            "{} instructions",
                                            block.sequence().len(),
                                        )),
                                    );

                                    flex.add(
                                        item(),
                                        egui::Label::new(format!(
                                            "{} of code",
                                            ByteSize(block.len() as u64)
                                        )),
                                    );

                                    flex.add_ui(item(), |ui| {
                                        ui.horizontal(|ui| {
                                            if ui.button("PPC").clicked() {
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

            let id = egui::Id::new("blocks-modal");
            let modal = egui::Modal::new(id).show(ui.ctx(), |ui| {
                ui.label(format!("Instructions: {}", block.sequence().len()));
                ui.allocate_ui(vec2(250.0, 400.0), |ui| {
                    egui::Frame::new()
                        .fill(colors().bg_color_dark)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                                    ui.style_mut().override_text_style =
                                        Some(egui::TextStyle::Monospace);
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
                });
            });

            if modal.should_close() {
                self.modal_mode = None;
            }
        }
    }
}
