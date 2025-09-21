use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender};
use events::{Event, EventResponse, EventType, UiEvent};
use busbar::{Copper, MakeT};
use engine::Engine;

use crate::utils::handle_engine_event;

use crate::Args;

/// Bring your crate types into scope as needed.
/// use crate::{events, handle_engine_event, Args, Engine, Event, EventResponse, EventType, UiEvent, Copper};
/// use qstudio_tcp;

pub struct QStudioServer {
    args: Args,

    engine_tx: Sender<Event>,
    engine_rx: Receiver<Event>,

    fs_tx: Sender<Event>,
    fs_rx: Receiver<Event>,

    dock_tx: Sender<Event>,
    dock_rx: Receiver<Event>,
}

pub struct ServerHandles {
    pub backend: JoinHandle<()>,
    pub filesystem: JoinHandle<()>,
    pub dock: JoinHandle<()>,
    pub engine: JoinHandle<()>,
}

impl QStudioServer {
    pub fn new(args: Args) -> Self {
        log::info!("Starting QStudio Server...");

        let (engine_tx, engine_rx) = crossbeam_channel::unbounded::<Event>();
        let (fs_tx, fs_rx) = crossbeam_channel::unbounded::<Event>();
        let (dock_tx, dock_rx) = crossbeam_channel::unbounded::<Event>();

        Self {
            args,
            engine_tx,
            engine_rx,
            fs_tx,
            fs_rx,
            dock_tx,
            dock_rx,
        }
    }

    /// Spawns ALL workers and returns their JoinHandles.
    pub fn start(&self) -> ServerHandles {
        let backend = self.spawn_backend_server();
        let filesystem = self.spawn_fs_listener();
        let dock = self.spawn_dock_listener();
        let engine = self.spawn_engine_worker();

        ServerHandles {
            backend,
            filesystem,
            dock,
            engine,
        }
    }

    /// Thread 1: TCP backend that receives from sockets and forwards into channels.
    pub fn spawn_backend_server(&self) -> JoinHandle<()> {
        let rx_address = self.args.rx_address.clone();
        let tx_address = self.args.tx_address.clone();

        // Clone the senders for routing incoming events
        let fs_tx = self.fs_tx.clone();
        let engine_tx = self.engine_tx.clone();
        let dock_tx = self.dock_tx.clone();

        thread::spawn(move || {
            log::info!("Starting Backend Server...");

            let mut txs = HashMap::new();
            txs.insert(events::EventType::FileEvent, fs_tx.clone());
            txs.insert(events::EventType::EngineEvent, engine_tx.clone());
            txs.insert(events::EventType::DockEvent, dock_tx.clone());
            // Add other mappings as your protocol grows.

            let server = qstudio_tcp::Server::new(rx_address, tx_address);
            server.listen::<EventType, Event, EventResponse>(txs);
        })
    }

    /// Thread 2: File-system worker: consumes fs events and may respond back to server.
    pub fn spawn_fs_listener(&self) -> JoinHandle<()> {
        let rx = self.fs_rx.clone();
        let tx_address = self.args.tx_address.clone();
        let root_dir = self.args.root_dir.clone();

        thread::spawn(move || {
            log::info!("Starting File System Listener...");
            let client = qstudio_tcp::Client::new(tx_address);

            loop {
                match rx.recv() {
                    Ok(event) => {
                        log::info!("File system event received: {}", event);
                        match event {
                            Event::FileEvent(file_event) => {
                                let root = std::path::PathBuf::from(&root_dir);
                                if let Some(response_event) = file_event.execute(&root) {
                                    if let Err(e) = client.send(Copper::ToServer {
                                        client_id: 0,
                                        payload: Event::FileEvent(response_event),
                                    }) {
                                        log::error!("Error sending file event response: {}", e);
                                    }
                                } else {
                                    log::warn!("No response from file event execution");
                                }
                            }
                            _ => {
                                log::warn!(
                                    "Received unsupported event type in file system listener"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("File system error receiving event: {}", e);
                        break;
                    }
                }
            }
        })
    }

    /// Thread 3: Dock worker: consumes dock events and sends UI updates back to server.
    pub fn spawn_dock_listener(&self) -> JoinHandle<()> {
        let rx = self.dock_rx.clone();
        let tx_address = self.args.tx_address.clone();

        thread::spawn(move || {
            log::info!("Starting Dock Listener...");
            let client = qstudio_tcp::Client::new(tx_address);

            loop {
                match rx.recv() {
                    Ok(event) => {
                        log::info!("Dock event received: {}", event);
                        match event {
                            Event::DockEvent(dock_event) => {
                                // Execute and return result to the server
                                let _ = client.send(Copper::ToServer {
                                    client_id: 0,
                                    payload: Event::DockEvent(dock_event.execute().clone()),
                                });
                            }
                            _ => {
                                log::warn!("Received unsupported event type in dock listener");
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Dock error receiving event: {}", e);
                        break;
                    }
                }
            }
        })
    }

    /// Thread 4: Engine worker: maintains engines, processes events, emits UI output changes.
    pub fn spawn_engine_worker(&self) -> JoinHandle<()> {
        let rx = self.engine_rx.clone();
        let tx_address = self.args.tx_address.clone();

        thread::spawn(move || {
            log::info!("Starting Engine...");
            let engines: Arc<Mutex<HashMap<String, Engine>>> =
                Arc::new(Mutex::new(HashMap::new()));
            let client = qstudio_tcp::Client::new(tx_address);

            let event_closure = |event: Event| match event {
                Event::EngineEvent(engine_event) => {
                    let notification =
                        handle_engine_event(engine_event, &mut engines.lock().unwrap());
                    match client.send(Copper::ToServer {
                        client_id: 0,
                        payload: notification.make_t(),
                    }) {
                        Ok(_) => log::info!("Engine event sent successfully"),
                        Err(e) => log::error!("Error sending engine event: {}", e),
                    }
                }
                _ => {
                    log::warn!("Engine received unsupported event type");
                }
            };

            let new_output_closure = || {
                let mut guard = engines.lock().unwrap();
                for (filename, engine) in guard.iter_mut() {
                    if engine.output_changed() {
                        if let Some(output) = engine.get_output() {
                            log::info!("Output updated for {}", filename);
                            if let Err(e) = client.send(Copper::ToServer {
                                client_id: 0,
                                payload: Event::UiEvent(UiEvent::NewOutputFromServer {
                                    filename: filename.clone(),
                                    output,
                                }),
                            }) {
                                log::error!("Error sending output notification: {}", e);
                            }
                        }
                    }
                }
            };

            loop {
                new_output_closure();
                match rx.recv() {
                    Ok(event) => {
                        log::info!("Engine received event: {}", event);
                        event_closure(event);
                    }
                    Err(e) => {
                        log::error!("Engine error receiving event: {}", e);
                        break;
                    }
                }
            }
        })
    }

    /// If you need access to the senders externally (optional helper):
    pub fn senders(&self) -> (Sender<Event>, Sender<Event>, Sender<Event>) {
        (self.fs_tx.clone(), self.engine_tx.clone(), self.dock_tx.clone())
    }
}

// Example usage:
//
// fn main() {
//     let args = parse_args(); // however you build Args
//     let server = QStudioServer::new(args);
//     let _handles = server.start();
//     // Optionally: _handles.backend.join().unwrap(); etc.
// }
