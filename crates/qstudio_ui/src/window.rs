use crate::components;
use crossbeam_channel::Sender;
use egui::ViewportCommand;
use qstudio_tcp::{Client, ClientList};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use busbar::{Aluminum, Copper};
use egui_notify::Toasts;
use events::events::notifications::{NotificationEvent, NotificationKind};
use events::{Event, EventResponse, EventType, UiEvent};

pub struct QStudioApp {
    aluminum: Arc<Aluminum<(Client, Event)>>,

    notification: Toasts,

    _id: String,

    topbar: components::topbar::TopBar,
    leftbar: components::leftbar::LeftBar,
    _bottombar: components::bottombar::BottomBar,
    rightbar: components::rightbar::RightBar,
    tabviewer: components::dock::MyTabViewer,
    center: components::dock::PaneDock,

    _client_list: Arc<Mutex<ClientList<Client>>>,
    _main_window_tx: Sender<crate::WindowRequest>,
}

impl QStudioApp {
    pub fn new(
        id: String,
        rx_address: String,
        tx_address: String,
        main_window_tx: Sender<crate::WindowRequest>,
    ) -> Self {
        let client_list: Arc<Mutex<ClientList<Client>>> = Arc::new(Mutex::new(ClientList::new()));

        let aluminum: Arc<Aluminum<(Client, Event)>> = Arc::new(Aluminum::new());
        client_list.lock().unwrap().add_client(
            "Main Window".into(),
            Client {
                addr: tx_address.clone(),
            },
        );

        let only_client = client_list
            .lock()
            .unwrap()
            .get_client("Main Window")
            .unwrap();

        log::info!("Starting QStudioApp with ID: {}", id);
        QStudioApp::listen(
            rx_address,
            tx_address,
            Arc::clone(&aluminum),
            Arc::clone(&client_list),
            main_window_tx.clone(),
            id.clone(),
        );
        Self {
            aluminum: Arc::clone(&aluminum),
            notification: Toasts::default(),
            _id: id,
            topbar: components::topbar::TopBar::new(Arc::clone(&aluminum), only_client.clone()),
            leftbar: components::leftbar::LeftBar::new(Arc::clone(&aluminum), only_client.clone()),
            rightbar: components::rightbar::RightBar::new(
                Arc::clone(&aluminum),
                only_client.clone(),
            ),
            _bottombar: components::bottombar::BottomBar::new(),
            tabviewer: components::dock::MyTabViewer::new(
                Arc::clone(&aluminum),
                only_client.clone(),
            ),
            center: components::dock::PaneDock::new(Arc::clone(&aluminum), only_client.clone()),
            _client_list: client_list,
            _main_window_tx: main_window_tx,
        }
    }
    fn listen(
        rx_address: String,
        tx_address: String,
        aluminum: Arc<Aluminum<(Client, Event)>>,
        client_list: Arc<Mutex<ClientList<Client>>>,
        main_window_tx: Sender<crate::WindowRequest>,
        id: String,
    ) {
        let aluminum = Arc::clone(&aluminum);

        // Clone only the sender part if needed, or refactor Aluminum to provide a sender clone.
        let backend_aluminum = Arc::clone(&aluminum);
        let tx_address_clone = tx_address.clone();
        let rx_address_clone = rx_address.clone();
        let client_list_clone = Arc::clone(&client_list);
        let id_clone = id.clone();
        thread::spawn(move || {
            let rx_address = rx_address_clone.clone();
            log::info!("Starting Frontend Server...");
            let client_list = Arc::clone(&client_list_clone);
            let mut txs: HashMap<events::EventType, Sender<(Client, Event)>> = HashMap::new();
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
            let rx_address_clone = rx_address.clone();

            log::info!("Listening on {}", rx_address_clone);
            let server_to_client =
                qstudio_tcp::Server::new(rx_address_clone.clone(), tx_address_clone.clone());
            qstudio_tcp::Server::new(rx_address_clone.clone(), tx_address_clone.clone());
            server_to_client
                .listen::<EventType, Event, EventResponse, Client>(txs, Arc::clone(&client_list));
        });

        thread::spawn({
            let backend_aluminum = Arc::clone(&aluminum);
            move || {
                log::info!("Starting Backend Aluminum Listener...");
                backend_aluminum.backend_listen();
            }
        });
        let rx_address_clone = rx_address.clone();
        let aluminum_clone = aluminum.clone();
        let arc_main_window_tx = Arc::new(main_window_tx);
        thread::spawn(move || {
            let rx_address = rx_address_clone.clone();

            let arc_main_window_tx = arc_main_window_tx.clone();
            log::info!("Starting UI Client...");
            loop {
                let (client, evt) = match aluminum_clone.frontend_rx.recv() {
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
                                        .send((client, Event::UiEvent(UiEvent::ToggleRightBar)))
                                        .unwrap_or_else(|e| {
                                            log::error!(
                                                "Failed to send ToggleRightBar event: {}",
                                                e
                                            );
                                        });
                                }
                                UiEvent::OpenNewWindow => {
                                    // Example action: Open a new window
                                    log::info!("Handling OpenNewWindow event");
                                    // Implement the logic to open a new window here
                                    arc_main_window_tx
                                        .send(crate::WindowRequest::OpenNew {
                                            preferred_id: Some(uuid::Uuid::new_v4().to_string()),
                                        })
                                        .unwrap_or_else(|e| {
                                            log::error!(
                                                "Failed to send OpenNewWindow request: {}",
                                                e
                                            );
                                        });
                                }
                                UiEvent::CloseWindow => {
                                    // Example action: Close the current window
                                    log::info!("Handling CloseWindow event");
                                    client
                                        .send::<String>(Copper::RemoveClient {
                                            client_id: id_clone.clone(),
                                            callback_address: rx_address.clone(),
                                        })
                                        .unwrap_or_else(|e| {
                                            log::error!("Error sending RemoveClient: {}", e);
                                        });
                                    // Implement the logic to close the current window here
                                    arc_main_window_tx
                                        .send(crate::WindowRequest::CloseWindow {
                                            id: id_clone.clone(),
                                        })
                                        .unwrap_or_else(|e| {
                                            log::error!(
                                                "Failed to send CloseWindow request: {}",
                                                e
                                            );
                                        });
                                }

                                UiEvent::ShowGraph { name } => {
                                    log::info!("Handling ShowGraph for: {}", name);
                                    // Implement the logic to show the graph in the UI
                                    aluminum_clone
                                        .dock_tx
                                        .send((
                                            client,
                                            Event::DockEvent(
                                                events::events::dock::DockEvent::ShowGraph { name },
                                            ),
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
                                        .send((
                                            client,
                                            Event::DockEvent(
                                                events::events::dock::DockEvent::ShowTrades {
                                                    name,
                                                },
                                            ),
                                        ))
                                        .unwrap_or_else(|e| {
                                            log::error!(
                                                "Failed to forward ShowTrades event: {}",
                                                e
                                            );
                                        });
                                }

                                UiEvent::NewOutputFromServer { filename, output } => {
                                    log::info!(
                                        "Handling NewOutputFromServer for file: {}",
                                        filename
                                    );
                                    // Implement the logic to update the UI with the new output
                                    aluminum_clone
                                        .dock_tx
                                        .send((
                                            client,
                                            Event::DockEvent(
                                                events::events::dock::DockEvent::UpdateOutput {
                                                    name: filename,
                                                    content: output,
                                                },
                                            ),
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
                        client_id: id_clone.clone(),
                        callback_address: rx_address.clone(),
                        payload: evt,
                    }) {
                        Ok(_) => log::info!("Event sent successfully"),
                        Err(e) => log::error!("Error sending event: {}", e),
                    }
                }
            }
        });
    }
    pub fn update(&mut self, ctx: &egui::Context) {
        // Draw first (lowest layer) once per frame:
        if ctx.input(|i| i.pointer.any_down()) && !ctx.wants_pointer_input() {
            // If the pointer is down and no widget wants it, start a drag the moment it moves
            // You can also put this behind a small movement threshold if you prefer.
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        self.topbar.ui(ctx);
        if let Ok((_client, not)) = self.aluminum.notification_rx.try_recv() {
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
                egui::Frame::NONE
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
