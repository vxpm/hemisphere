pub mod cpu;

use crate::emulator::State;
use cpu::CpuTab;
use eframe::egui;
use egui_dock::{DockState, TabViewer, tab_viewer::OnCloseResponse};
use slotmap::{SlotMap, new_key_type};

pub trait Tab {
    fn title(&mut self) -> egui::WidgetText;
    fn ui(&mut self, state: &mut State, ui: &mut egui::Ui);

    fn is_closeable(&self) -> bool {
        true
    }
}

pub type BoxedTab = Box<dyn Tab>;

new_key_type! {
    pub struct TabId;
}

pub struct Viewer<'a> {
    pub tabs: &'a mut SlotMap<TabId, BoxedTab>,
    pub state: &'a mut State,
}

impl<'a> TabViewer for Viewer<'a> {
    type Tab = TabId;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        self.tabs[*tab].title()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        self.tabs[*tab].ui(self.state, ui)
    }

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        self.tabs[*tab].is_closeable()
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> OnCloseResponse {
        self.tabs.remove(*tab);
        OnCloseResponse::Close
    }
}

pub struct Manager {
    pub tabs: SlotMap<TabId, BoxedTab>,
    pub dock: DockState<TabId>,
}

impl Default for Manager {
    fn default() -> Self {
        let mut tabs: SlotMap<TabId, BoxedTab> = SlotMap::with_key();
        let mut dock = DockState::new(vec![]);
        "Undock".clone_into(&mut dock.translations.tab_context_menu.eject_button);

        let control_tab = tabs.insert(Box::new(CpuTab {}));

        dock.main_surface_mut()
            .root_node_mut()
            .unwrap()
            .append_tab(control_tab);

        // let [a, b] =
        //     dock_state
        //         .main_surface_mut()
        //         .split_left(NodeIndex::root(), 0.3, vec![control_tab]);
        //
        // let [_, _] = dock_state.main_surface_mut().split_below(
        //     a,
        //     0.7,
        //     vec!["File Browser".to_owned(), "Asset Manager".to_owned()],
        // );
        //
        // let [_, _] =
        //     dock_state
        //         .main_surface_mut()
        //         .split_below(b, 0.5, vec!["Hierarchy".to_owned()]);

        Self { tabs, dock }
    }
}
