extern crate qstudiov3;

use eframe::*;
use egui_notify::Toasts;

use engine::controllers::Output;
use engine::Engine;
use qstudiov3::models::notification::Notification;
use qstudiov3::utils::match_file_extension_for_pane_type;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use qstudiov3::models::engine::EngineEvent;
use qstudiov3::models::ui::UIEvent;
use qstudiov3::Channels;

use qstudiov3::views::dock::{MyTabViewer, PaneDock};
use qstudiov3::views::sidebar::SideBar;

pub struct State {
    notification: Toasts,
    engine: Arc<Mutex<HashMap<String, Arc<Mutex<Engine>>>>>,
    dataframes: Arc<Mutex<HashMap<String, Arc<Output>>>>,

    channels: Arc<Channels>,

    sidebar: SideBar,

    tab: MyTabViewer,
    dock: PaneDock,
}

impl State {
    pub fn new(channels: Arc<Channels>) -> Self {
        let dataframes: Arc<Mutex<HashMap<String, Arc<Output>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let engine = Arc::new(Mutex::new(HashMap::new()));
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
                    .notification_tx
                    .lock()
                    .unwrap()
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
            .notification_tx
            .lock()
            .unwrap()
            .send(Notification::Success(format!(
                "Engine created for file: {}",
                file_path
            )))
            .unwrap();
    }
}

impl eframe::App for State {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(engine_event) = self.channels.engine_rx.try_recv().ok() {
            match engine_event {
                EngineEvent::Start(file_path) => {
                    if let Some(engine) = self.engine.lock().unwrap().get(&file_path) {
                        let mut engine = engine.lock().unwrap();
                        match engine.analyze() {
                            Ok(_) => {
                                if let Ok(out) = tokio::runtime::Runtime::new()
                                    .unwrap()
                                    .block_on(engine.run())
                                {
                                    self.dataframes
                                        .lock()
                                        .unwrap()
                                        .insert(file_path.clone(), Arc::new(out));
                                }
                                self.channels
                                    .notification_tx
                                    .lock()
                                    .unwrap()
                                    .send(Notification::Success(format!(
                                        "Engine started for file: {}",
                                        file_path
                                    )))
                                    .unwrap();
                            }
                            Err(e) => {
                                self.channels
                                    .notification_tx
                                    .lock()
                                    .unwrap()
                                    .send(Notification::Error(format!(
                                        "Failed to start engine: {}",
                                        e
                                    )))
                                    .unwrap();
                            }
                        }
                    } else {
                        self.channels
                            .notification_tx
                            .lock()
                            .unwrap()
                            .send(Notification::Error(format!(
                                "No engine found for file: {}",
                                file_path
                            )))
                            .unwrap();
                    }
                }

                EngineEvent::Stop(file_path) => {
                    if let Some(engine) = self.engine.lock().unwrap().get(&file_path) {
                        let mut engine = engine.lock().unwrap();
                        // engine.status() = engine::EngineStatus::Stopped;
                        self.channels
                            .notification_tx
                            .lock()
                            .unwrap()
                            .send(Notification::Success(format!(
                                "Engine stopped for file: {}",
                                file_path
                            )))
                            .unwrap();
                    } else {
                        self.channels
                            .notification_tx
                            .lock()
                            .unwrap()
                            .send(Notification::Error(format!(
                                "No engine found for file: {}",
                                file_path
                            )))
                            .unwrap();
                    }
                }

                EngineEvent::UpdateSource(file_path) => {
                    if let Some(engine) = self.engine.lock().unwrap().get(&file_path) {
                        let mut engine = engine.lock().unwrap();
                        if let Err(e) = engine.update_code() {
                            self.channels
                                .notification_tx
                                .lock()
                                .unwrap()
                                .send(Notification::Error(format!(
                                    "Failed to update engine code: {}",
                                    e
                                )))
                                .unwrap();
                        } else {
                            self.channels
                                .notification_tx
                                .lock()
                                .unwrap()
                                .send(Notification::Success(format!(
                                    "Engine source updated for file: {}",
                                    file_path
                                )))
                                .unwrap();
                        }
                    } else {
                        self.channels
                            .notification_tx
                            .lock()
                            .unwrap()
                            .send(Notification::Error(format!(
                                "No engine found for file: {}",
                                file_path
                            )))
                            .unwrap();
                    }
                }

                EngineEvent::Restart(file_path) => {
                    if let Some(engine) = self.engine.lock().unwrap().get(&file_path) {
                        let mut engine = engine.lock().unwrap();
                        if let Err(e) = engine.analyze() {
                            self.channels
                                .notification_tx
                                .lock()
                                .unwrap()
                                .send(Notification::Error(format!(
                                    "Failed to analyze engine: {}",
                                    e
                                )))
                                .unwrap();
                        } else {
                            self.channels
                                .notification_tx
                                .lock()
                                .unwrap()
                                .send(Notification::Success(format!(
                                    "Engine restarted for file: {}",
                                    file_path
                                )))
                                .unwrap();
                        }
                    } else {
                        self.channels
                            .notification_tx
                            .lock()
                            .unwrap()
                            .send(Notification::Error(format!(
                                "No engine found for file: {}",
                                file_path
                            )))
                            .unwrap();
                    }
                }
            }
        }

        if let Some(event) = self.channels.ui_rx.try_recv().ok() {
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

                UIEvent::Update => {
                    ctx.request_repaint();
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

        egui::SidePanel::left("sidebar")
            .frame(
                egui::Frame::none()
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
            .frame(egui::Frame::none().inner_margin(0.0).outer_margin(0.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                // self.pane_tree.ui(ui);
                self.dock.ui(ui, &mut self.tab);
            });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::none().inner_margin(0.0).outer_margin(0.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                // self.status_bar.ui(ui);
            });

        self.notification.show(ctx);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };

    let (notification_tx, notification_rx) = std::sync::mpsc::channel::<Notification>();
    let (ui_tx, ui_rx) = std::sync::mpsc::channel::<UIEvent>();
    let (engine_tx, engine_rx) = std::sync::mpsc::channel::<EngineEvent>();

    let ui_tx = Arc::new(Mutex::new(ui_tx));
    let thread_ui_tx = Arc::clone(&ui_tx);
    thread::spawn(move || {
        while let Ok(notification) = notification_rx.recv() {
            let _ = Arc::clone(&thread_ui_tx)
                .lock()
                .unwrap()
                .send(UIEvent::Notification(notification))
                .unwrap();
        }
    });

    let channels = Arc::new(Channels {
        ui_rx,
        ui_tx: Arc::clone(&ui_tx),
        notification_tx: Arc::new(Mutex::new(notification_tx)),
        engine_rx: Arc::new(engine_rx),
        engine_tx: Arc::new(Mutex::new(engine_tx)),
    });

    eframe::run_native(
        "Q Studio",
        options,
        Box::new(|cc| {
            egui_material_icons::initialize(&cc.egui_ctx);
            Ok(Box::new(State::new(Arc::clone(&channels))))
        }),
    )
}
