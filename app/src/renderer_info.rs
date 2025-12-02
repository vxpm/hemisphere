use crate::{Ctx, State, windows::AppWindow};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::cell::Cell;

type RenderDoc = renderdoc::RenderDoc<renderdoc::V140>;

thread_local! {
    static COUNT: Cell<u32> = Cell::new(0);
}

#[derive(Serialize, Deserialize)]
pub struct Window {
    #[serde(skip)]
    is_canonical: bool,
    #[serde(skip)]
    renderdoc: Option<RenderDoc>,
    #[serde(skip)]
    capture: bool,
    #[serde(skip)]
    is_capturing: bool,
}

impl Default for Window {
    fn default() -> Self {
        let count = COUNT.get();
        COUNT.set(count + 1);

        Self {
            is_canonical: count == 0,
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

    fn prepare(&mut self, _: &mut State) {}

    fn show(&mut self, ui: &mut egui::Ui, ctx: &mut Ctx) {
        ui.set_max_width(150.0);

        if !self.is_canonical {
            if COUNT.get() == 0 {
                COUNT.set(1);
                self.is_canonical = true;
            }

            ui.label("Only one renderer window should exist!");
            return;
        }

        if let Some(renderdoc) = &mut self.renderdoc {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.capture, "Capture (Renderdoc)");
                if self.capture {
                    renderdoc.trigger_capture();
                }
            });
        } else {
            self.renderdoc = RenderDoc::new().ok();
            ui.label("Renderdoc not detected");
        }
    }
}
