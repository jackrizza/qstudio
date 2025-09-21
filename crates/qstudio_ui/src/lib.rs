// lib.rs (or your top-level GUI file)

mod components;
mod window; // defines QStudioApp

use crossbeam_channel::{Receiver, Sender};
use eframe::{self, egui};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::window::QStudioApp;

// ---------- Requests coming from any window to the main ----------
#[derive(Debug, Clone)]
enum WindowRequest {
    /// Ask the main app to open a new window. Optional human-readable hint/id.
    OpenNew { preferred_id: Option<String> },
}

// ---------- Per-window record held by the main ----------
struct WindowRecord {
    id: String,
    viewport_id: egui::ViewportId,
    app: Arc<Mutex<QStudioApp>>, // <-- interior mutability
    is_main: bool,
}

// ---------- Main application ----------
pub struct QStudioUI {
    windows: HashMap<String, WindowRecord>,
    main_id: String,

    // Create-window plumbing
    create_tx: Sender<WindowRequest>,
    create_rx: Receiver<WindowRequest>,

    // Networking bits
    base_rx_host: String, // e.g. "127.0.0.1"
    base_rx_port: u16,    // e.g. 7879
    tx_address: String,   // unchanged for all windows (your original design)
    used_ports: HashSet<u16>,
}

impl QStudioUI {
    pub fn new(rx_address: String, tx_address: String) -> Self {
        let (create_tx, create_rx) = crossbeam_channel::unbounded::<WindowRequest>();

        let (host, port) = split_host_port(&rx_address, 7879);
        let mut used_ports = HashSet::new();
        used_ports.insert(port);

        // Build main window first
        let main_id = "Main".to_string();
        let main_viewport = egui::ViewportId::ROOT; // root/main OS window
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
        // Fallback: Window-1, Window-2, ...
        let mut i = 1usize;
        loop {
            let candidate = format!("Window-{}", i);
            if !self.windows.contains_key(&candidate) {
                return candidate;
            }
            i += 1;
        }
    }

    fn handle_requests(&mut self) {
        while let Ok(msg) = self.create_rx.try_recv() {
            match msg {
                WindowRequest::OpenNew { preferred_id } => {
                    let id = self.ensure_unique_id(preferred_id.as_deref());
                    self.create_deferred_window(id);
                }
            }
        }
    }

    fn create_deferred_window(&mut self, id: String) {
        // Allocate a unique port & addresses
        let port = self.next_free_port();
        let rx_address = format!("{}:{}", self.base_rx_host, port);

        // Build child app with a clone of create_tx so it can request new windows too
        let app = Arc::new(Mutex::new(QStudioApp::new(
            id.clone(),
            rx_address,
            self.tx_address.clone(),
            self.create_tx.clone(),
        )));

        // Create a stable viewport id for this window
        let viewport_id = egui::ViewportId::from_hash_of(&id);

        self.windows.insert(
            id.clone(),
            WindowRecord {
                id,
                viewport_id,
                app,
                is_main: false,
            },
        );
        log::info!("Created deferred window");
    }
}

impl eframe::App for QStudioUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1) Drain any window-create requests first (deterministic mutations)
        self.handle_requests();

        // 2) Draw MAIN window (root viewport)
        if let Some(main) = self.windows.get(&self.main_id) {
            debug_assert!(main.is_main);
            if let Ok(mut app) = main.app.lock() {
                app.update(ctx);
            }
        }

        // 3) Draw all DEFERRED windows
        //    First, collect the data we need into a temporary vector,
        //    so we don’t hold a &mut borrow into self.windows inside the closure.
        let mut to_render: Vec<(egui::ViewportId, String, Arc<Mutex<QStudioApp>>)> = Vec::new();
        for w in self.windows.values().filter(|w| !w.is_main) {
            to_render.push((
                w.viewport_id,
                format!("QStudio – {}", w.id),
                Arc::clone(&w.app),
            ));
        }

        // 4) Now spawn each deferred viewport with a 'static, move closure
        for (viewport_id, title, app_arc) in to_render {
            ctx.show_viewport_deferred(
                viewport_id,
                egui::ViewportBuilder::default()
                    .with_title(title)
                    .with_inner_size([1280.0, 720.0])
                    .with_inner_size([1280.0, 720.0])
                    .with_decorations(false)
                    .with_transparent(false),
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

// ---------- Helper: parse "host:port" safely ----------
fn split_host_port(addr: &str, default_port: u16) -> (String, u16) {
    match addr.rsplit_once(':') {
        Some((host, port_str)) => {
            let p = port_str.parse::<u16>().unwrap_or(default_port);
            (host.to_string(), p)
        }
        None => (addr.to_string(), default_port),
    }
}

// ---------- Top-level runner ----------
pub fn window(rx_address: String, tx_address: String) -> eframe::Result<()> {
    let app = QStudioUI::new(rx_address, tx_address);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("QStudio")
            .with_inner_size([1280.0, 720.0])
            .with_inner_size([1280.0, 720.0])
            .with_decorations(false)
            .with_transparent(false),
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
