use crate::{emulator::State, tab::Tab};
use eframe::egui::{self, CollapsingHeader};

pub struct CpuTab {}

impl CpuTab {
    fn user_regs(&self, state: &mut State, ui: &mut eframe::egui::Ui) {
        let regs = &state.emulator.cpu.user;
        CollapsingHeader::new("GPR").show(ui, |ui| {
            for (gpr, value) in regs.gpr.iter().copied().enumerate() {
                if value == 0 {
                    continue;
                }

                ui.label(format!("{gpr:02}: 0x{value:08X}"));
            }

            ui.label("...")
                .on_hover_text("Registers not shown have a zeroed bit pattern");
        });

        CollapsingHeader::new("FPR").show(ui, |ui| {
            for (fpr, value) in regs.fpr.iter().copied().enumerate() {
                if value == 0.0 {
                    continue;
                }

                ui.label(format!("{fpr:02}: 0x{:016X}", value as u64));
            }

            ui.label("...")
                .on_hover_text("Registers not shown have a zeroed bit pattern");
        });

        CollapsingHeader::new(format!("CR: 0x{:08X}", regs.cr.to_bits())).show(ui, |ui| {
            let fields = regs.cr.fields();
            let iter = fields.iter().copied().rev().enumerate();

            for (index, cond) in iter {
                ui.label(format!("CR{index}: {cond:?}"));
            }
        });

        CollapsingHeader::new(format!("XER: 0x{:08X}", regs.xer.to_bits())).show(ui, |ui| {
            ui.label(format!("{:?}", regs.xer));
        });

        ui.label(format!("CTR: 0x{:08X}", regs.ctr));
        ui.label(format!("LR: 0x{:08X}", regs.lr));
    }
}

impl Tab for CpuTab {
    fn title(&mut self) -> eframe::egui::WidgetText {
        "CPU View".into()
    }

    fn ui(&mut self, state: &mut State, ui: &mut eframe::egui::Ui) {
        ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
        ui.label(format!("PC: 0x{:08X}", state.emulator.pc.value()));

        CollapsingHeader::new("User").show(ui, |ui| {
            self.user_regs(state, ui);
        });
    }
}
