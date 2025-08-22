use crate::tab::{Context, Tab};
use eframe::egui::{self, RichText};
use egui_extras::{Column, TableBuilder};
use tinylog::record::RecordWithCtx;

const MAX_RECORDS_SHOWN: usize = 1_000;

#[derive(Default)]
pub struct LogsTab {
    buffer: Vec<RecordWithCtx>,
    current_ctx: Option<tinylog::logger::Context>,
}

impl Tab for LogsTab {
    fn title(&mut self) -> eframe::egui::WidgetText {
        "Logs".into()
    }

    fn ui(&mut self, ctx: Context, ui: &mut eframe::egui::Ui) {
        let current_logctx = self
            .current_ctx
            .get_or_insert_with(|| ctx.loggers.contexts()[0].clone())
            .clone();

        let records = ctx.records;
        let len = records.len(current_logctx.clone());

        let start = len.saturating_sub(MAX_RECORDS_SHOWN);
        let end = len;

        self.buffer.clear();
        records.get_range(&current_logctx, start..end, &mut self.buffer);

        egui::Sides::new().show(
            ui,
            |_| (),
            |ui| {
                egui::ComboBox::from_label("Context:")
                    .selected_text(&current_logctx.to_string())
                    .show_ui(ui, |ui| {
                        let mut changed = false;
                        for context in ctx.loggers.contexts() {
                            let context_str = context.to_string();
                            if ui
                                .selectable_value(&mut self.current_ctx, Some(context), context_str)
                                .clicked()
                            {
                                changed = true;
                            }
                        }

                        changed
                    });
            },
        );

        TableBuilder::new(ui)
            .column(Column::auto().at_least(115.0).resizable(false))
            .column(Column::auto().at_least(50.0).resizable(false))
            .column(Column::remainder())
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.label("Time");
                });
                header.col(|ui| {
                    ui.label("Context");
                });
                header.col(|ui| {
                    ui.label("Message");
                });
            })
            .body(|body| {
                body.rows(15.0, self.buffer.len(), |mut row| {
                    let index = row.index();
                    let record = &self.buffer[index];

                    row.col(|ui| {
                        let time =
                            RichText::new(record.value.time().format("%H:%M:%S%.6f").to_string())
                                .monospace()
                                .weak();

                        ui.label(time);
                    });

                    row.col(|ui| {
                        ui.label(format!("{}", record.ctx));
                    });

                    row.col(|ui| {
                        ui.label(format!("{}", record.value.message));
                    });
                });
            });
    }
}
