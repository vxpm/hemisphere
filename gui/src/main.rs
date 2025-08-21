mod tab;

use crate::tab::Tab;
use crate::tab::control::ControlTab;
use eframe::egui::{self, Color32};
use egui::{CentralPanel, Frame, TopBottomPanel, Ui, ViewportBuilder, WidgetText, vec2};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState, TabViewer};
use slotmap::SlotMap;
use slotmap::new_key_type;
use std::sync::Arc;

type BoxedTab = Box<dyn Tab>;

new_key_type! {
    struct TabId;
}

#[derive(Default)]
struct Context {
    tabs: SlotMap<TabId, BoxedTab>,
}

impl Context {
    pub fn new(tabs: SlotMap<TabId, BoxedTab>) -> Self {
        Self { tabs }
    }
}

impl TabViewer for Context {
    type Tab = TabId;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        self.tabs[*tab].title()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        self.tabs[*tab].ui(ui)
    }

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        self.tabs[*tab].is_closeable()
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> OnCloseResponse {
        self.tabs.remove(*tab);
        OnCloseResponse::Close
    }
}

struct App {
    context: Context,
    tree: DockState<TabId>,
}

impl Default for App {
    fn default() -> Self {
        let mut tabs: SlotMap<TabId, BoxedTab> = SlotMap::with_key();
        let mut dock_state = DockState::new(vec![]);
        "Undock".clone_into(&mut dock_state.translations.tab_context_menu.eject_button);

        let control_tab = tabs.insert(Box::new(ControlTab {}));

        dock_state
            .main_surface_mut()
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

        Self {
            context: Context::new(tabs),
            tree: dock_state,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::top("hemisphere_menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("View", |ui| {});
            })
        });

        CentralPanel::default()
            .frame(Frame::central_panel(&ctx.style()).inner_margin(0.))
            .show(ctx, |ui| {
                DockArea::new(&mut self.tree)
                    .show_close_buttons(true)
                    .show_add_buttons(true)
                    .draggable_tabs(true)
                    .show_inside(ui, &mut self.context);
            });
    }
}

fn visuals() -> egui::Visuals {
    let hex = |s| Color32::from_hex(s).unwrap();

    // background - gamecube purple
    let outline = hex("#302950");
    let bg_color_light = hex("#1B172C");
    let bg_color = hex("#151223");
    let bg_color_dark = hex("#0B0912");

    // text - ash white
    let text = hex("#CFBEC6");

    let widget = |bg_fill| egui::style::WidgetVisuals {
        bg_fill,
        weak_bg_fill: bg_fill,
        bg_stroke: egui::Stroke {
            color: outline,
            ..Default::default()
        },
        fg_stroke: egui::Stroke {
            color: text,
            ..Default::default()
        },
        corner_radius: egui::CornerRadius {
            nw: 2,
            ne: 2,
            sw: 2,
            se: 2,
        },
        expansion: 0.0,
    };

    egui::Visuals {
        dark_mode: true,
        window_fill: bg_color,
        panel_fill: bg_color,
        window_stroke: egui::Stroke {
            color: outline,
            ..Default::default()
        },

        faint_bg_color: bg_color_light,
        extreme_bg_color: bg_color_dark,
        code_bg_color: bg_color_dark,

        widgets: egui::style::Widgets {
            noninteractive: widget(bg_color_light),
            inactive: widget(bg_color_light),
            hovered: widget(bg_color),
            active: widget(bg_color),
            ..Default::default()
        },

        ..Default::default()
    }
}

// fn configure_text_styles(ctx: &egui::Context) {
//     use egui::FontFamily;
//     use egui::FontId;
//     use egui::TextStyle::*;
//
//     ctx.style_mut(|style| {
//         style.text_styles = [
//             (Heading, FontId::new(30.0, FontFamily::Proportional)),
//             (Body, FontId::new(18.0, FontFamily::Proportional)),
//             (Monospace, FontId::new(14.0, FontFamily::Monospace)),
//             (Button, FontId::new(14.0, FontFamily::Proportional)),
//             (Small, FontId::new(10.0, FontFamily::Proportional)),
//         ]
//         .into();
//     });
// }

fn main() -> eframe::Result<()> {
    let instance = wgpu::InstanceDescriptor::from_env_or_default();
    let mut native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size(vec2(1024.0, 1024.0)),
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        ..Default::default()
    };

    native_options.viewport.min_inner_size = Some(egui::Vec2::new(500.0, 500.0));
    native_options.wgpu_options.wgpu_setup =
        eframe::egui_wgpu::WgpuSetup::CreateNew(eframe::egui_wgpu::WgpuSetupCreateNew {
            instance_descriptor: instance,
            power_preference: wgpu::PowerPreference::HighPerformance,
            native_adapter_selector: None,
            device_descriptor: Arc::new(|_| wgpu::DeviceDescriptor {
                label: Some("device"),
                required_features: wgpu::Features::default(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            }),
        });

    eframe::run_native(
        "hemisphere",
        native_options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(visuals());
            cc.egui_ctx.set_zoom_factor(1.25);

            Ok(Box::<App>::default())
        }),
    )
}
