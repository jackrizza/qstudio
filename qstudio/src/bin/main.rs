extern crate qstudiov3;

use env_logger;

use eframe::*;
use egui_notify::Toasts;

use engine::controllers::Output;
use engine::Engine;
use qstudiov3::models::notification::{self, Notification};
use qstudiov3::utils::match_file_extension_for_pane_type;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use qstudiov3::models::engine::EngineEvent;
use qstudiov3::models::ui::UIEvent;
use qstudiov3::{Channels, Receivers, Senders};

use qstudiov3::views::dock::{MyTabViewer, PaneDock};
use qstudiov3::views::searchbar::SearchBar;
use qstudiov3::views::sidebar::SideBar;

use dotenv::dotenv;
use std::env;

pub struct State {
    notification: Toasts,
    engine: Arc<Mutex<HashMap<String, Arc<Mutex<Engine>>>>>,
    dataframes: Arc<Mutex<HashMap<String, Arc<Output>>>>,

    channels: Arc<Channels>,

    sidebar: SideBar,
    searchbar: SearchBar,

    tab: MyTabViewer,
    dock: PaneDock,
}

impl State {
    pub fn new(
        channels: Arc<Channels>,
        dataframes: Arc<Mutex<HashMap<String, Arc<Output>>>>,
        engine: Arc<Mutex<HashMap<String, Arc<Mutex<Engine>>>>>,
    ) -> Self {
        let tab = MyTabViewer::new(Arc::clone(&dataframes), Arc::clone(&channels));
        let sidebar = SideBar::new(".".to_string(), Arc::clone(&engine));
        State {
            notification: Toasts::default(),
            engine,
            dataframes, // Initialize with an empty HashMap

            channels,

            sidebar,
            tab,
            dock: PaneDock::new(),
            searchbar: SearchBar::new(),
        }
    }

    fn engine_insert(&mut self, file_path: String, engine: Arc<Mutex<Engine>>) {
        self.engine.lock().unwrap().insert(file_path, engine);
    }

    pub fn add_engine(&mut self, file_path: String) {
        if file_path.is_empty() || file_path.split(".").last() != Some("qql") {
            return;
        }
        let engine = match Engine::new(&file_path) {
            Ok(engine) => Arc::new(Mutex::new(engine)),
            Err(e) => {
                self.channels
                    .senders()
                    .notification_tx()
                    .send(Notification::Error(format!(
                        "Failed to create engine: {}",
                        e
                    )))
                    .unwrap();
                return;
            }
        };
        self.engine_insert(file_path.clone(), engine);
        self.channels
            .senders()
            .notification_tx()
            .send(Notification::Success(format!(
                "Engine created for file: {}",
                file_path
            )))
            .unwrap();
    }
}

impl eframe::App for State {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // self.channels.log_channel_events();

        if let Ok(event) = self.channels.receivers.ui_rx() {
            match event {
                UIEvent::Notification(notification) => {
                    notification.create_toast(&mut self.notification);
                }

                UIEvent::AddPane(pane) => {
                    let t = pane.title().to_string();
                    let pane = match_file_extension_for_pane_type(&pane, &t);

                    self.add_engine(pane.title().to_string());
                    self.dock.add_pane(pane, &mut self.tab);
                }
                UIEvent::RemovePane(title) => {
                    self.dock.remove_pane(&title);
                }
                UIEvent::Update => {
                    ctx.request_repaint();
                }

                UIEvent::SearchBarMode(mode) => {
                    self.searchbar.search_mode = mode;
                    self.sidebar.show_search = true;
                }
            }
        }

        let visuals = ctx.style().visuals.clone();
        let is_dark_mode = visuals.dark_mode;
        let primary_background = if is_dark_mode {
            egui::Color32::from_rgb(30, 30, 30)
        } else {
            egui::Color32::from_rgb(240, 240, 240)
        };

        if self.sidebar.show_search {
            self.searchbar.ui(ctx);
        } else {
            self.searchbar.reset();
        }

        egui::SidePanel::left("sidebar")
            .frame(
                egui::Frame::new()
                    .inner_margin(0.0)
                    .outer_margin(0.0)
                    .fill(primary_background),
            )
            .min_width(self.sidebar.width())
            .max_width(self.sidebar.width())
            .resizable(false)
            .show_animated(ctx, true, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                self.sidebar.ui(ui, Arc::clone(&self.channels));
            });

        // Main pane tree (code editor, graph, etc.)
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(0.0).outer_margin(0.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                // self.pane_tree.ui(ui);
                self.dock.ui(ui, &mut self.tab);
            });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::new().inner_margin(0.0).outer_margin(0.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                // self.status_bar.ui(ui);
            });

        self.notification.show(ctx);
    }
}

fn main() -> eframe::Result<()> {
    dotenv().ok();

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("Starting Q Studio...");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };

    let mut channels = Channels::new();
    let dataframes: Arc<Mutex<HashMap<String, Arc<Output>>>> = Arc::new(Mutex::new(HashMap::new()));
    let engine = Arc::new(Mutex::new(HashMap::new()));

    channels.notification_thread();
    channels.engine_thread(Arc::clone(&engine), Arc::clone(&dataframes));

    eframe::run_native(
        "Q Studio",
        options,
        Box::new(|cc| {
            egui_material_icons::initialize(&cc.egui_ctx);
            Ok(Box::new(State::new(
                Arc::new(channels),
                Arc::clone(&dataframes),
                Arc::clone(&engine),
            )))
        }),
    )
}
