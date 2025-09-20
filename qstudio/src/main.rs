extern crate qstudio;

use busbar::{Copper, MakeT};
use clap::Parser;
use events::{Event, EventResponse, EventType, UiEvent};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use qstudio::utils::handle_engine_event;

use engine::Engine;

/// Simple program to greet a person
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    /// server recieving tcp stream address
    #[arg(long, default_value_t = String::from("127.0.0.1:7878"))]
    rx_address: String,
    /// server transmitting tcp stream address
    #[arg(long, default_value_t = String::from("127.0.0.1:7879"))]
    tx_address: String,

    /// run client
    #[arg(short, long, default_value_t = false)]
    client: bool,
    /// run server
    #[arg(short, long, default_value_t = false)]
    server: bool,

    /// Root directory for file system watcher
    #[arg(short, long, default_value_t = String::from("."))]
    root_dir: String,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    if !args.client && !args.server {
        log::error!("Please specify --client or --server to run the respective mode.");
        log::warn!("Also, you can run both with --client --server for a local only setup.");
        log::warn!("Refer to --help for more information.");
        std::process::exit(1);
    }

    if args.server.clone() {
        server(args.clone());
    }
    if args.client.clone() {
        client(args.clone());
    }

    std::thread::park();
}

fn client(args: Args) {
    log::info!("Starting UI...");
    match qstudio_ui::window(args.tx_address, args.rx_address) {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            log::error!("Error starting UI: {}", e);
            std::process::exit(0);
        }
    }
}

fn server(args: Args) {
    log::info!("Starting QStudio Server...");

    let (engine_tx, engine_rx) = crossbeam_channel::unbounded::<Event>();
    let (fs_tx, fs_rx) = crossbeam_channel::unbounded::<Event>();
    let (dock_tx, dock_rx) = crossbeam_channel::unbounded::<Event>();

    thread::spawn({
        let rx_address = args.rx_address.clone();
        let tx_address = args.tx_address.clone();
        move || {
            log::info!("Starting Backend Server...");

            let mut txs = HashMap::new();
            txs.insert(events::EventType::FileEvent, fs_tx.clone());
            txs.insert(events::EventType::EngineEvent, engine_tx.clone());
            txs.insert(events::EventType::DockEvent, dock_tx.clone());
            // Add other event types and their corresponding senders as needed.

            let server = qstudio_tcp::Server::new(rx_address, tx_address);
            server.listen::<EventType, Event, EventResponse>(txs);
        }
    });
    let args_clone = args.clone();
    thread::spawn({
        move || {
            log::info!("Starting File System Listener...");
            let client = qstudio_tcp::Client::new(args_clone.tx_address.clone());
            loop {
                match fs_rx.recv() {
                    Ok(event) => {
                        log::info!("File system event received: {}", event);
                        // Handle file system events here
                        match event {
                            Event::FileEvent(file_event) => {
                                let root = std::path::PathBuf::from(&args_clone.root_dir);
                                if let Some(response_event) = file_event.execute(&root) {
                                    // You can send the response_event to another channel if needed
                                    client
                                        .send(Copper::ToServer {
                                            client_id: 0,
                                            payload: Event::FileEvent(response_event),
                                        })
                                        .unwrap_or_else(|e| {
                                            log::error!("Error sending file event response: {}", e)
                                        });
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
        }
    });

    let args_clone = args.clone();
    thread::spawn({
        move || {
            log::info!("Starting Dock Listener...");
            let client = qstudio_tcp::Client::new(args_clone.tx_address.clone());
            loop {
                match dock_rx.recv() {
                    Ok(event) => {
                        log::info!("Dock event received: {}", event);
                        // Handle dock events here
                        match event {
                            Event::DockEvent(dock_event) => {
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
        }
    });

    thread::spawn({
        move || {
            log::info!("Starting Engine...");
            let engines: Arc<Mutex<HashMap<String, Engine>>> = Arc::new(Mutex::new(HashMap::new()));
            let client = qstudio_tcp::Client::new(args.tx_address.clone());

            let event_closure = |event| match event {
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
                for (filename, engine) in engines.lock().unwrap().iter_mut() {
                    if engine.output_changed() {
                        if let Some(output) = engine.get_output() {
                            log::info!("Output updated for {}", filename);
                            match client.send(Copper::ToServer {
                                client_id: 0,
                                payload: Event::UiEvent(UiEvent::NewOutputFromServer {
                                    filename: filename.clone(),
                                    output,
                                }),
                            }) {
                                Ok(_) => log::info!("Output notification sent successfully"),
                                Err(e) => log::error!("Error sending output notification: {}", e),
                            }
                        }
                    }
                }
            };

            loop {
                new_output_closure();
                match engine_rx.recv() {
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
        }
    });
}
