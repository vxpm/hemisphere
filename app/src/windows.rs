use crate::{Ctx, State};
use eframe::egui;

#[typetag::serde]
pub(crate) trait AppWindow: 'static {
    fn title(&self) -> &str;
    fn prepare(&mut self, state: &mut State);
    fn show(&mut self, ui: &mut egui::Ui, ctx: &mut Ctx);
}
