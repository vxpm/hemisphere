use crate::Ctx;
use eframe::egui;
use hemisphere::runner::State;

#[typetag::serde]
pub(crate) trait AppWindow: 'static {
    fn title(&self) -> &str;
    fn prepare(&mut self, state: &mut State);
    fn show(&mut self, ui: &mut egui::Ui, ctx: &mut Ctx);
}
