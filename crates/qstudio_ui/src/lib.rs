mod components;

use events::events::engine::EngineEvent;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use busbar::{Aluminum, Copper};
use eframe::wgpu::naga::front;
use egui_notify::Toasts;
use events::events::notifications::{NotificationEvent, NotificationKind};
use events::{Event, EventResponse, EventType};
use qstudio_tcp::Client;

pub struct Window {
    aluminum: Arc<Aluminum<Event>>,

    notification: Toasts,

    topbar: components::topbar::TopBar,
    leftbar: components::leftbar::LeftBar,
}

impl Window {
    pub fn new(aluminum: Arc<Aluminum<events::Event>>) -> Self {
        Self {
            aluminum,
            notification: Toasts::default(),
            topbar: components::topbar::TopBar::new(),
            leftbar: components::leftbar::LeftBar::new(),
        }
    }
}

impl eframe::App for Window {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // self.topbar.ui(ctx);
        if let Ok(not) = self.aluminum.notification_rx.try_recv() {
            log::info!("UI received notification event: {}", not);
            match not {
                Event::NotificationEvent(n) => match n.kind {
                    NotificationKind::Info => {
                        create_toast(NotificationKind::Info, n.message, &mut self.notification);
                    }
                    NotificationKind::Warning => {
                        create_toast(NotificationKind::Warning, n.message, &mut self.notification);
                    }
                    NotificationKind::Error => {
                        create_toast(NotificationKind::Error, n.message, &mut self.notification);
                    }
                },
                _ => {}
            }
        }
        self.leftbar.ui(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Hello, World!");
            if ui.button("Click me").clicked() {
                let _ = self
                    .aluminum
                    .frontend_tx
                    .send(Event::EngineEvent(EngineEvent::Start {
                        filename: "./test queries/test.qql".into(),
                    }));
            }
        });

        self.notification.show(ctx);
    }
}

pub fn window(rx_address: String, tx_address: String) -> eframe::Result<()> {
    let aluminum: Aluminum<Event> = Aluminum::new();
    let aluminum = Arc::new(aluminum);

    // Clone only the sender part if needed, or refactor Aluminum to provide a sender clone.
    let backend_aluminum = aluminum.clone() as Arc<Aluminum<Event>>;
    let tx_address_clone = tx_address.clone();

    thread::spawn(move || {
        log::info!("Starting Frontend Server...");

        let mut txs = HashMap::new();
        txs.insert(
            events::EventType::UiEvent,
            backend_aluminum.frontend_tx.clone(),
        );
        txs.insert(
            events::EventType::NotificationEvent,
            backend_aluminum.notification_tx.clone(),
        );

        // Add other event types and their corresponding senders as needed.

        let server_to_client =
            qstudio_tcp::Server::new(rx_address.clone(), tx_address_clone.clone());
        server_to_client.listen::<EventType, Event, EventResponse>(txs);
    });

    thread::spawn({
        let backend_aluminum = aluminum.clone() as Arc<Aluminum<Event>>;
        move || {
            log::info!("Starting Backend Aluminum Listener...");
            backend_aluminum.backend_listen();
        }
    });

    let tx_address = tx_address.clone();
    let aluminum_clone = aluminum.clone();
    thread::spawn(move || {
        log::info!("Starting UI Client...");
        let client = qstudio_tcp::Client::new(tx_address.clone());
        loop {
            match aluminum_clone.frontend_rx.recv() {
                Ok(event) => {
                    log::info!("Client sending event: {}", event);
                    match client.send(Copper::ToServer {
                        client_id: 1,
                        payload: event,
                    }) {
                        Ok(_) => log::info!("Event sent successfully"),
                        Err(e) => log::error!("Error sending event: {}", e),
                    }
                }
                Err(e) => {
                    log::error!("Client error receiving event: {}", e);
                    break;
                }
            }
        }
    });

    // If Window only needs to send events, pass a sender clone; otherwise, refactor as needed.
    // Here, we assume Window does not need the receiver.
    let window = Window::new(Arc::clone(&aluminum));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([1280.0, 720.0]),
        // .with_movable_by_background(true)
        // .with_decorations(false),
        // .with_transparent(true), // To have rounded corners we need transparency
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "QStudio",
        options,
        Box::new(|cc| {
            egui_material_icons::initialize(&cc.egui_ctx);
            Ok(Box::new(window))
        }),
    )
}

pub fn create_toast(kind: NotificationKind, msg: String, notification: &mut Toasts) {
    match kind {
        NotificationKind::Info => {
            notification.success(msg);
        }
        NotificationKind::Warning => {
            notification.warning(msg);
        }
        NotificationKind::Error => {
            notification.error(msg);
        }
    }
}
