use crate::WindowUi;
use eframe::egui;
use hemisphere::runner::State;

#[derive(Default)]
pub struct Window {
    texture: Option<egui::TextureHandle>,
}

impl WindowUi for Window {
    fn build<'open>(&mut self, open: &'open mut bool) -> egui::Window<'open> {
        egui::Window::new("XFB").open(open)
    }

    fn show(&mut self, ui: &mut eframe::egui::Ui, state: &mut State) {
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

        let hemisphere = state.hemisphere();
        if !hemisphere.system.bus.video.regs.display_config.enable() {
            ui.label("VI disabled");
            return;
        }

        let resolution = hemisphere.system.bus.video.xfb_resolution();
        if resolution.0 == 0 || resolution.1 == 0 {
            ui.label("VI bad resolution");
            return;
        }

        let mut rgb = Vec::new();
        hemisphere.system.dump_xfb(&mut rgb);

        let mut rgba = Vec::with_capacity(rgb.len() / 3);
        for rgb in rgb.chunks_exact(3) {
            rgba.push(egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
        }

        let size = [resolution.0 as usize, resolution.1 as usize];
        let source_size = egui::Vec2::new(size[0] as f32, size[1] as f32);
        texture.set(
            egui::ColorImage {
                size,
                source_size,
                pixels: rgba,
            },
            egui::TextureOptions::LINEAR,
        );

        let size = texture.size_vec2();
        let sized_texture = egui::load::SizedTexture::new(texture, size);
        ui.add(egui::Image::new(sized_texture).fit_to_exact_size(size));
    }
}
