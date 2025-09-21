// lib.rs

mod components;
mod window;

use crossbeam_channel::{Receiver, Sender};
use eframe::{self, egui};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::window::QStudioApp;

// Requests from any window to the main
#[derive(Debug, Clone)]
enum WindowRequest {
    OpenNew { preferred_id: Option<String> },
    CloseWindow { id: String },
}

// Per-window record
struct WindowRecord {
    id: String,
    viewport_id: egui::ViewportId,
    app: Arc<Mutex<QStudioApp>>,
    port: u16,          // <-- track the port so we can free it
    is_main: bool,
}

pub struct QStudioUI {
    windows: HashMap<String, WindowRecord>,
    main_id: String,

    create_tx: Sender<WindowRequest>,
    create_rx: Receiver<WindowRequest>,

    base_rx_host: String,
    base_rx_port: u16,
    tx_address: String,
    used_ports: HashSet<u16>,
}

impl QStudioUI {
    pub fn new(rx_address: String, tx_address: String) -> Self {
        let (create_tx, create_rx) = crossbeam_channel::unbounded::<WindowRequest>();

        let (host, port) = split_host_port(&rx_address, 7879);
        let mut used_ports = HashSet::new();
        used_ports.insert(port);

        let main_id = Uuid::new_v4().to_string();
        let main_viewport = egui::ViewportId::ROOT;

        let main_app = Arc::new(Mutex::new(QStudioApp::new(
            main_id.clone(),
            format!("{}:{}", host, port),
            tx_address.clone(),
            create_tx.clone(),
        )));

        let mut windows = HashMap::new();
        windows.insert(
            main_id.clone(),
            WindowRecord {
                id: main_id.clone(),
                viewport_id: main_viewport,
                app: main_app,
                port,
                is_main: true,
            },
        );

        Self {
            windows,
            main_id,
            create_tx,
            create_rx,
            base_rx_host: host,
            base_rx_port: port,
            tx_address,
            used_ports,
        }
    }

    fn next_free_port(&mut self) -> u16 {
        let mut p = self.base_rx_port.max(1);
        loop {
            p = p.saturating_add(1);
            if !self.used_ports.contains(&p) {
                self.used_ports.insert(p);
                return p;
            }
        }
    }

    fn ensure_unique_id(&self, preferred: Option<&str>) -> String {
        if let Some(p) = preferred {
            if !self.windows.contains_key(p) {
                return p.to_string();
            }
        }
        let mut i = 1usize;
        loop {
            let candidate = format!("Window-{}", i);
            if !self.windows.contains_key(&candidate) {
                return candidate;
            }
            i += 1;
        }
    }

    fn handle_requests(&mut self, ctx: &egui::Context) {
        let mut to_close: Vec<String> = Vec::new();

        while let Ok(msg) = self.create_rx.try_recv() {
            match msg {
                WindowRequest::OpenNew { preferred_id } => {
                    let id = self.ensure_unique_id(preferred_id.as_deref());
                    self.create_deferred_window(id);
                }
                WindowRequest::CloseWindow { id } => {
                    // Remember to close after the loop so we don’t mutate while iterating elsewhere
                    to_close.push(id);
                }
            }
        }

        // Actually remove the windows and free their ports
        for id in to_close {
            if let Some(rec) = self.windows.remove(&id) {
                self.used_ports.remove(&rec.port);
                // Ensure the OS window is closed in case this was programmatic
                // (If the user clicked the OS “X”, it’s already requested close; calling again is harmless)
                ctx.send_viewport_cmd_to(rec.viewport_id, egui::ViewportCommand::Close);
                log::info!("Closed deferred window {id}");
            }
        }
    }

    fn create_deferred_window(&mut self, id: String) {
        let port = self.next_free_port();
        let rx_address = format!("{}:{}", self.base_rx_host, port);

        let app = Arc::new(Mutex::new(QStudioApp::new(
            id.clone(),
            rx_address,
            self.tx_address.clone(),
            self.create_tx.clone(),
        )));

        let viewport_id = egui::ViewportId::from_hash_of(&id);

        self.windows.insert(
            id.clone(),
            WindowRecord {
                id,
                viewport_id,
                app,
                port,     // track the port
                is_main: false,
            },
        );
        log::info!("Created deferred window");
    }
}

impl eframe::App for QStudioUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1) Drain requests (including Close) first
        self.handle_requests(ctx);

        // 2) Main/root
        if let Some(main) = self.windows.get(&self.main_id) {
            debug_assert!(main.is_main);
            if let Ok(mut app) = main.app.lock() {
                app.update(ctx);
            }
        }

        // 3) Deferred: collect first, then show with 'static closures
        let mut to_render: Vec<(egui::ViewportId, String, Arc<Mutex<QStudioApp>>)> = Vec::new();
        for w in self.windows.values().filter(|w| !w.is_main) {
            to_render.push((
                w.viewport_id,
                format!("QStudio – {}", w.id),
                Arc::clone(&w.app),
            ));
        }

        for (viewport_id, title, app_arc) in to_render {
            ctx.show_viewport_deferred(
                viewport_id,
                egui::ViewportBuilder::default()
                    .with_title(title)
                    .with_inner_size([1280.0, 720.0])
                    .with_decorations(false)
                    .with_transparent(true),
                move |ctx, class| {
                    assert!(
                        class == egui::ViewportClass::Deferred,
                        "Backend must support multiple viewports"
                    );
                    if let Ok(mut app) = app_arc.lock() {
                        app.update(ctx);
                    }
                },
            );
        }
    }
}

fn split_host_port(addr: &str, default_port: u16) -> (String, u16) {
    match addr.rsplit_once(':') {
        Some((host, port_str)) => {
            let p = port_str.parse::<u16>().unwrap_or(default_port);
            (host.to_string(), p)
        }
        None => (addr.to_string(), default_port),
    }
}

pub fn window(rx_address: String, tx_address: String) -> eframe::Result<()> {
    let app = QStudioUI::new(rx_address, tx_address);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("QStudio")
            .with_inner_size([1280.0, 720.0])
            .with_decorations(false)
            .with_transparent(true),
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "QStudio",
        options,
        Box::new(|cc| {
            egui_material_icons::initialize(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}
