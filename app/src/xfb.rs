use crate::{Ctx, WindowUi};
use eframe::egui;
use hemisphere::runner::State;

#[inline]
fn ycbcr_to_rgb(y: u8, cb: u8, cr: u8) -> [u8; 3] {
    let (y, cb, cr) = (y as f32, cb as f32 - 128.0, cr as f32 - 128.0);

    let r = y + 1.371 * cr;
    let g = y - 0.698 * cr - 0.336 * cb;
    let b = y + 1.732 * cb;

    [r, g, b].map(|x| x.clamp(0.0, 255.0) as u8)
}

pub struct Window {
    bottom: bool,
    texture: Option<egui::TextureHandle>,
}

impl Window {
    pub fn new() -> Self {
        Self {
            bottom: false,
            texture: None,
        }
    }
}

impl WindowUi for Window {
    fn title(&self) -> &str {
        "🖵 XFB"
    }

    fn show(&mut self, ui: &mut egui::Ui, _: &mut Ctx, state: &mut State) {
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

        let core = state.core();
        if !core.system.video.display_config.enable() {
            ui.label("VI disabled");
            return;
        }

        let resolution = core.system.video.xfb_resolution();
        if resolution.0 == 0 || resolution.1 == 0 {
            ui.label("VI bad resolution");
            return;
        }

        let Some(xfb) = (if self.bottom {
            core.system.bottom_xfb()
        } else {
            core.system.top_xfb()
        }) else {
            ui.label("XFB data error");
            return;
        };

        let mut pixels = Vec::with_capacity(xfb.len() / 2);
        for ycbcr in xfb.chunks_exact(4) {
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
