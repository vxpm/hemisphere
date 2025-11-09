use crate::{Ctx, State, windows::AppWindow};
use eframe::egui;
use serde::{Deserialize, Serialize};

#[inline]
fn ycbcr_to_rgb(y: u8, cb: u8, cr: u8) -> [u8; 3] {
    let (y, cb, cr) = (y as f32, cb as f32 - 128.0, cr as f32 - 128.0);

    let r = y + 1.371 * cr;
    let g = y - 0.698 * cr - 0.336 * cb;
    let b = y + 1.732 * cb;

    [r, g, b].map(|x| x.clamp(0.0, 255.0) as u8)
}

#[derive(Default, Serialize, Deserialize)]
pub struct Window {
    #[serde(skip)]
    bottom: bool,
    #[serde(skip)]
    xfb_enabled: bool,
    #[serde(skip)]
    xfb_resolution: (u16, u16),
    #[serde(skip)]
    xfb_data: Vec<u8>,
    #[serde(skip)]
    texture: Option<egui::TextureHandle>,
}

#[typetag::serde(name = "xfb")]
impl AppWindow for Window {
    fn title(&self) -> &str {
        "🖵 XFB"
    }

    fn prepare(&mut self, state: &mut State) {
        let emulator = &state.emulator;
        if !emulator.system.video.display_config.enable() {
            self.xfb_enabled = false;
            return;
        }

        self.xfb_resolution = emulator.system.video.xfb_resolution();
        let Some(xfb) = (if self.bottom {
            emulator.system.bottom_xfb()
        } else {
            emulator.system.top_xfb()
        }) else {
            return;
        };

        self.xfb_data.clear();
        self.xfb_data.extend_from_slice(xfb);
    }

    fn show(&mut self, ui: &mut egui::Ui, _: &mut Ctx) {
        let texture = match &mut self.texture {
            Some(tex) => tex,
            None => {
                let tex = ui.ctx().load_texture(
                    "xfb",
                    egui::ColorImage::example(),
                    egui::TextureOptions::LINEAR,
                );
                self.texture = Some(tex.clone());
                self.texture.as_mut().unwrap()
            }
        };

        let resolution = self.xfb_resolution;
        if resolution.0 == 0 || resolution.1 == 0 {
            ui.label("VI bad resolution");
            return;
        }

        let mut pixels = Vec::with_capacity(self.xfb_data.len() / 2);
        for ycbcr in self.xfb_data.chunks_exact(4) {
            let [r, g, b] = ycbcr_to_rgb(ycbcr[0], ycbcr[1], ycbcr[3]);
            pixels.push(egui::Color32::from_rgb(r, g, b));

            let [r, g, b] = ycbcr_to_rgb(ycbcr[2], ycbcr[1], ycbcr[3]);
            pixels.push(egui::Color32::from_rgb(r, g, b));
        }

        let size = [resolution.0 as usize, resolution.1 as usize];
        let source_size = egui::Vec2::new(size[0] as f32, size[1] as f32);
        texture.set(
            egui::ColorImage {
                size,
                source_size,
                pixels,
            },
            egui::TextureOptions::LINEAR,
        );

        let size = texture.size_vec2();
        let sized_texture = egui::load::SizedTexture::new(texture, size);
        ui.add(egui::Image::new(sized_texture).fit_to_exact_size(size));
    }
}
