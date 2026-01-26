use std::cell::Cell;

use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::State;
use crate::windows::{AppWindow, Ctx};

type RenderDoc = renderdoc::RenderDoc<renderdoc::V140>;

#[derive(Serialize, Deserialize)]
pub struct Window {
    #[serde(skip)]
    renderdoc: Option<RenderDoc>,
    #[serde(skip)]
    capture: bool,
    #[serde(skip)]
    is_capturing: bool,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            renderdoc: RenderDoc::new().ok(),
            capture: false,
            is_capturing: false,
        }
    }
}

#[typetag::serde(name = "renderer_info")]
impl AppWindow for Window {
    fn title(&self) -> &str {
        "Renderer"
    }

    fn prepare(&mut self, state: &mut State) {}

    fn show(&mut self, ui: &mut egui::Ui, ctx: &mut Ctx) {
        let stats = ctx.renderer.stats();

        if let Some(renderdoc) = &mut self.renderdoc {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.capture, "Capture (Renderdoc)");

                let null = std::ptr::null();
                if self.is_capturing {
                    if ctx.renderer.rendered_anything() {
                        renderdoc.end_frame_capture(null, null);
                        self.is_capturing = false;
                    } else {
                        renderdoc.discard_frame_capture(null, null);
                        renderdoc.start_frame_capture(null, null);
                    }
                }

                if self.capture && !self.is_capturing {
                    ctx.renderer.rendered_anything();
                    renderdoc.start_frame_capture(null, null);
                    self.is_capturing = true;
                }
            });
        } else {
            self.renderdoc = RenderDoc::new().ok();
            ui.label("Renderdoc not detected");
        }
    }
}
