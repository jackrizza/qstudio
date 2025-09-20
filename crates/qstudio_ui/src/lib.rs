mod components;

use egui::{Stroke, Ui, ViewportCommand};
use events::events::engine::EngineEvent;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use busbar::{Aluminum, Copper};
use egui_notify::Toasts;
use events::events::notifications::{NotificationEvent, NotificationKind};
use events::{Event, EventResponse, EventType, UiEvent};

pub struct Window {
    aluminum: Arc<Aluminum<Event>>,

    notification: Toasts,

    topbar: components::topbar::TopBar,
    leftbar: components::leftbar::LeftBar,
    bottombar: components::bottombar::BottomBar,
    rightbar: components::rightbar::RightBar,
    tabviewer: components::dock::MyTabViewer,
    center: components::dock::PaneDock,
}

impl Window {
    pub fn new(aluminum: Arc<Aluminum<events::Event>>) -> Self {
        Self {
            aluminum: Arc::clone(&aluminum),
            notification: Toasts::default(),
            topbar: components::topbar::TopBar::new(Arc::clone(&aluminum)),
            leftbar: components::leftbar::LeftBar::new(Arc::clone(&aluminum)),
            rightbar: components::rightbar::RightBar::new(Arc::clone(&aluminum)),
            bottombar: components::bottombar::BottomBar::new(),
            tabviewer: components::dock::MyTabViewer::new(Arc::clone(&aluminum)),
            center: components::dock::PaneDock::new(Arc::clone(&aluminum)),
        }
    }
}

impl eframe::App for Window {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Draw first (lowest layer) once per frame:
        if ctx.input(|i| i.pointer.any_down()) && !ctx.wants_pointer_input() {
            // If the pointer is down and no widget wants it, start a drag the moment it moves
            // You can also put this behind a small movement threshold if you prefer.
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        self.topbar.ui(ctx);
        if let Ok(not) = self.aluminum.notification_rx.try_recv() {
            log::info!("UI received notification event: {}", not);
            let notification = match not {
                Event::NotificationEvent(notification) => notification,
                _ => {
                    log::warn!("UI received unsupported event type for notification");
                    NotificationEvent {
                        kind: NotificationKind::Warning,
                        message: "Unsupported event type for notification".into(),
                    }
                }
            };
            create_toast(
                notification.kind,
                notification.message,
                &mut self.notification,
            );
        }

        self.leftbar.ui(ctx);

        self.rightbar.ui(ctx);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .inner_margin(0.0)
                    .outer_margin(0.0)
                    .fill(theme::get_mode_theme(ctx).base), // .stroke(Stroke::new(0.5, egui::Color32::BLACK)),
            )
            .show(ctx, |ui| {
                ui.set_max_width(ui.available_width() - self.rightbar.width);
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                self.center.ui(ui, &mut self.tabviewer);
            });

        // self.bottombar.ui(ctx);
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

        txs.insert(
            events::EventType::FileEvent,
            backend_aluminum.filetree_tx.clone(),
        );
        txs.insert(
            events::EventType::DockEvent,
            backend_aluminum.dock_tx.clone(),
        );
        txs.insert(
            events::EventType::EngineEvent,
            backend_aluminum.engine_tx.clone(),
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
            let evt = match aluminum_clone.frontend_rx.recv() {
                Ok(event) => event,
                Err(e) => {
                    log::error!("Client error receiving event: {}", e);
                    break;
                }
            };

            if evt.event_type() == EventType::UiEvent {
                log::info!("UI Client received event: {}", evt);
                match evt {
                    Event::UiEvent(ui_event) => {
                        match ui_event {
                            // Handle specific UiEvent variants here
                            // For example:
                            // UiEvent::SomeAction { data } => { ... }
                            UiEvent::ToggleRightBar => {
                                // Example action: Toggle the visibility of the right bar
                                log::info!("Handling ToggleRightBar event");
                                // Implement the logic to toggle the right bar here
                                aluminum_clone
                                    .widget_tx
                                    .send(Event::UiEvent(UiEvent::ToggleRightBar))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send ToggleRightBar event: {}", e);
                                    });
                            }
                            UiEvent::OpenNewWindow => {
                                // Example action: Open a new window
                                log::info!("Handling OpenNewWindow event");
                                // Implement the logic to open a new window here
                            }

                            UiEvent::ShowGraph { name } => {
                                log::info!("Handling ShowGraph for: {}", name);
                                // Implement the logic to show the graph in the UI
                                aluminum_clone
                                    .dock_tx
                                    .send(Event::DockEvent(
                                        events::events::dock::DockEvent::ShowGraph { name },
                                    ))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to forward ShowGraph event: {}", e);
                                    });
                            }

                            UiEvent::ShowTrades { name } => {
                                log::info!("Handling ShowTrades for: {}", name);
                                // Implement the logic to show the trades in the UI
                                aluminum_clone
                                    .dock_tx
                                    .send(Event::DockEvent(
                                        events::events::dock::DockEvent::ShowTrades { name },
                                    ))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to forward ShowTrades event: {}", e);
                                    });
                            }

                            UiEvent::NewOutputFromServer { filename, output } => {
                                log::info!("Handling NewOutputFromServer for file: {}", filename);
                                // Implement the logic to update the UI with the new output
                                aluminum_clone
                                    .dock_tx
                                    .send(Event::DockEvent(
                                        events::events::dock::DockEvent::UpdateOutput {
                                            name: filename,
                                            content: output,
                                        },
                                    ))
                                    .unwrap_or_else(|e| {
                                        log::error!(
                                            "Failed to forward NewOutputFromServer event: {}",
                                            e
                                        );
                                    });
                            } // Add handling for other UiEvent variants as needed

                            _ => {
                                log::warn!("Received unhandled UiEvent variant");
                            }
                        }
                    }

                    _ => {
                        log::warn!("Received non-UiEvent in UI Client");
                    }
                }
            } else {
                log::info!("UI Client sending event to server: {}", evt.event_type());
                match client.send(Copper::ToServer {
                    client_id: 1,
                    payload: evt,
                }) {
                    Ok(_) => log::info!("Event sent successfully"),
                    Err(e) => log::error!("Error sending event: {}", e),
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
            .with_min_inner_size([1280.0, 720.0])
            // .with_movable_by_background(true)
            .with_decorations(false)
            .with_transparent(true), // To have rounded corners we need transparency
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 0,
        multisampling: 0,
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
