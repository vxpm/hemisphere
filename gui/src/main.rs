mod emulator;
mod tab;

use crate::{emulator::Emulator, tab::Viewer};
use eframe::egui::{self, Color32};
use egui_dock::DockArea;
use std::{sync::Arc, time::Duration};

struct Colors {
    outline_light: Color32,
    outline: Color32,
    bg_color_light: Color32,
    bg_color: Color32,
    bg_color_dark: Color32,
    text: Color32,
}

fn colors() -> &'static Colors {
    static COLORS: std::sync::LazyLock<Colors> = std::sync::LazyLock::new(|| {
        let hex = |s| Color32::from_hex(s).unwrap();
        Colors {
            outline_light: hex("#3B3361"),
            outline: hex("#302950"),
            bg_color_light: hex("#2b2547"),
            bg_color: hex("#151223"),
            bg_color_dark: hex("#0B0912"),
            text: hex("#CFBEC6"),
        }
    });

    &COLORS
}

struct App {
    tabs: tab::Manager,
    emulator: Emulator,
}

impl App {
    fn new() -> Self {
        Self {
            tabs: tab::Manager::default(),
            emulator: Emulator::new(),
        }
    }
}

fn dock_style(ui: &egui::Ui) -> egui_dock::Style {
    let mut style = egui_dock::Style::from_egui(ui.style().as_ref());
    style.tab.tab_body.stroke = egui::Stroke::NONE;

    style
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("hemisphere_menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                egui::Sides::new().show(
                    ui,
                    |ui| {
                        ui.menu_button("View", |_| {});
                    },
                    |ui| {
                        let height = ui.available_height();
                        let (text, action) = if self.emulator.running() {
                            ("⏸", Emulator::stop as fn(_))
                        } else {
                            ("▶", Emulator::run as fn(_))
                        };

                        let button = egui::Button::new(text).min_size(egui::vec2(height, height));
                        if ui.add(button).clicked() {
                            action(&mut self.emulator);
                        }
                    },
                );
            });
        });

        let mut state = self.emulator.state();
        let mut viewer = Viewer {
            tabs: &mut self.tabs.tabs,
            state: &mut state,
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(0.))
            .show(ctx, |ui| {
                DockArea::new(&mut self.tabs.dock)
                    .style(dock_style(ui))
                    .show_close_buttons(true)
                    .show_add_buttons(true)
                    .draggable_tabs(true)
                    .show_inside(ui, &mut viewer);
            });

        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn visuals() -> egui::Visuals {
    let Colors {
        outline_light,
        outline,
        bg_color_light,
        bg_color,
        bg_color_dark,
        text,
    } = *colors();

    let widget = |bg_fill| egui::style::WidgetVisuals {
        bg_fill,
        weak_bg_fill: bg_fill,
        bg_stroke: egui::Stroke::NONE,
        fg_stroke: egui::Stroke {
            color: text,
            width: 1.0,
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
            width: 1.0,
        },

        faint_bg_color: bg_color_light,
        extreme_bg_color: bg_color_dark,
        code_bg_color: bg_color_dark,

        widgets: egui::style::Widgets {
            noninteractive: {
                egui::style::WidgetVisuals {
                    bg_stroke: egui::Stroke {
                        color: outline,
                        width: 0.5,
                    },
                    ..widget(bg_color_light)
                }
            },
            inactive: widget(bg_color_light),
            hovered: egui::style::WidgetVisuals {
                bg_stroke: egui::Stroke {
                    color: outline_light,
                    width: 1.25,
                },
                ..widget(bg_color_light)
            },
            active: widget(bg_color_light),
            ..Default::default()
        },

        selection: egui::style::Selection {
            bg_fill: outline_light,
            stroke: egui::Stroke {
                width: 1.0,
                color: text,
            },
        },

        indent_has_left_vline: true,
        striped: true,

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
        viewport: egui::ViewportBuilder::default().with_inner_size(egui::vec2(1024.0, 1024.0)),
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
            cc.egui_ctx.options_mut(|options| {
                options.max_passes = std::num::NonZero::new(5).unwrap();
            });

            Ok(Box::new(App::new()))
        }),
    )
}
