use crate::{Ctx, windows::AppWindow};
use eframe::{
    egui::{self, Vec2},
    egui_wgpu::{self, CallbackTrait},
};
use hemisphere::runner::State;
use renderer::WgpuRenderer;
use serde::{Deserialize, Serialize};

pub struct RendererCallback {
    renderer: WgpuRenderer,
}

impl CallbackTrait for RendererCallback {
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut eframe::wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
    ) {
        self.renderer.render(render_pass);
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Window;

#[typetag::serde(name = "efb")]
impl AppWindow for Window {
    fn title(&self) -> &str {
        "🖵 EFB"
    }

    fn prepare(&mut self, _: &mut State) {}

    fn show(&mut self, ui: &mut egui::Ui, ctx: &mut Ctx) {
        ui.take_available_space();
        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            let aspect_ratio = 4.0 / 3.0;
            let available_height = (ui.available_height() - 20.0).max(0.0);

            let rect = if ui.available_width() < available_height {
                ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), ui.available_width() / aspect_ratio),
                    egui::Sense::click(),
                )
                .0
            } else {
                ui.allocate_exact_size(
                    Vec2::new(available_height * aspect_ratio, available_height),
                    egui::Sense::click(),
                )
                .0
            };

            ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                rect,
                RendererCallback {
                    renderer: ctx.renderer.clone(),
                },
            ));
        });
    }
}
