#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(rustdoc::missing_crate_level_docs)]

mod graph;
use eframe::egui;
use egui_dock::{DockArea, DockState, OverlayType, Style, TabAddAlign};
use engine::Engine;
use rand::distr::Alphanumeric;
use rand::Rng;
use std::cell::RefCell;
use std::rc::Rc;
use theme::set_theme;
use theme::{GITHUB_DARK, GITHUB_LIGHT};

mod preview;
use preview::Preview;

mod menubar;
use menubar::menubar;

mod models;
use models::{QueryResult, Settings};

mod views;
use views::editor::{MyTabViewer, Tab, TabKind};

use engine::controllers::Output;

pub fn random_string() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect()
}
fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Q Studio",
        options,
        Box::new(|_cc| Ok(Box::<QStudio>::default())),
    )
}

pub struct QStudio {
    dock_state: DockState<Tab>,
    tab_viewer: MyTabViewer,
    query_result: Result<Output, String>,
    debug_panel: bool,
    settings: Rc<RefCell<Settings>>,
    preview: Preview,
}

impl Default for QStudio {
    fn default() -> Self {
        let settings = Rc::new(RefCell::new(Settings { dark_mode: true }));

        let tabs = [TabKind::Markdown("0".to_string())].into_iter().collect();

        QStudio {
            dock_state: DockState::new(tabs),
            tab_viewer: MyTabViewer::new(settings.clone()),
            query_result: Ok(Output::default()),
            debug_panel: false,
            settings,
            preview: Preview::default(),
        }
    }
}

impl eframe::App for QStudio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        set_theme(
            ctx,
            if self.settings.borrow().dark_mode {
                GITHUB_DARK
            } else {
                GITHUB_LIGHT
            },
        );

        // Handle Command+R (Run Query)
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::R)) {
            self.run_query();
        }

        // Handle Command+S (Save Code)
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            if let Some(tab) = self.dock_state.find_active_focused() {
                if let TabKind::Code(key) = tab.1 {
                    if let Err(e) = self.tab_viewer.save_code(&key) {
                        eprintln!("Failed to save code: {}", e);
                    }
                }
            }
        }

        self.code_panel(ctx);
        match &self.query_result {
            Ok(Output::Data {
                graph,
                tables,
                trades,
            }) => {
                self.preview
                    .render(ctx, graph.clone(), tables.clone(), trades.clone());
            }
            Err(ref e) | Ok(Output::Error(ref e)) => {
                self.preview.error(ctx, e.clone());
            }
            _ => {}
        }

        if self.debug_panel {
            self.debug_panel(ctx);
        }
    }
}

impl QStudio {
    fn debug_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("Debug Panel")
            .resizable(true)
            .default_width(300.0)
            .min_width(200.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Debug Information");
                    if ui.button("Close").clicked() {
                        self.debug_panel = false;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_width(400.0)
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        if let Some(tab) = self.dock_state.find_active_focused() {
                            if let TabKind::Code(key) = tab.1 {
                                if let Some(code) = self.tab_viewer.open_code_tab(&key) {
                                    ui.collapsing("Engine Query", |ui| {
                                        match Engine::new(code.code.as_str()) {
                                            Ok(engine) => {
                                                ui.label(format!(
                                                    "Engine Query: {:#?}",
                                                    engine.query()
                                                ));
                                            }
                                            Err(e) => {
                                                ui.label(format!(
                                                    "Engine initialization error: {}",
                                                    e
                                                ));
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        ui.collapsing("Dock State", |ui| {
                            ui.label(format!("{:#?}", self.dock_state));
                        });
                        ui.collapsing("Tab Viewer Data", |ui| {
                            ui.label(format!("{:#?}", self.tab_viewer.data));
                        });
                        ui.collapsing("Query Result", |ui| {
                            ui.label(format!("{:#?}", self.query_result));
                        });
                    });
            });
    }

    fn code_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("Code Editor")
            .resizable(true)
            .default_width(ctx.screen_rect().width() / 2.0)
            .min_width(ctx.screen_rect().width() / 4.0)
            .show(ctx, |ui| {
                menubar(ui, self);

                let mut style = Style::from_egui(ui.style());
                style.overlay.overlay_type = OverlayType::HighlightedAreas;
                style.buttons.add_tab_align = TabAddAlign::Left;
                style.tab_bar.fill_tab_bar = true;

                DockArea::new(&mut self.dock_state)
                    .style(style)
                    .show_inside(ui, &mut self.tab_viewer);
            });
    }

    fn run_query(&mut self) {
        // println!("Running query...");
        // println!("{:?}", self.dock_state.find_active_focused());
        if let Some(tab) = self.dock_state.find_active_focused() {
            // println!("Running query in tab: {}", tab.1.as_str());
            if let TabKind::Code(key) = tab.1 {
                if let Some(code) = self.tab_viewer.open_code_tab(&key) {
                    // Clone the code string so no references escape
                    let code_string = code.code.clone();
                    // println!("Running query: {}", code_string);
                    let thread = std::thread::spawn(move || {
                        // If Engine::run is async, you need to use a runtime here
                        let engine = Engine::new(code_string.as_str());
                        match engine {
                            Ok(mut engine) => {
                                // If run() is async, use a runtime to block on it
                                let result = tokio::runtime::Runtime::new()
                                    .unwrap()
                                    .block_on(engine.run());
                                result
                            }
                            Err(e) => Err(format!("Failed to create engine: {}", e)),
                        }
                    });

                    self.query_result = thread.join().unwrap();
                }
            }
        }
    }
}
