extern crate qstudio;

use busbar::{Aluminum, Copper};
use clap::Parser;
use events::events::notifications::{NotificationEvent, NotificationKind};
use events::{Event, EventResponse, EventType};
use std::collections::HashMap;
use std::thread;

use qstudio::utils::handle_engine_event;

use engine::Engine;

/// Simple program to greet a person
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    /// server recieving tcp stream address
    #[arg(short, long, default_value_t = String::from("127.0.0.1:7878"))]
    rx_address: String,
    /// server transmitting tcp stream address
    #[arg(short, long, default_value_t = String::from("127.0.0.1:7879"))]
    tx_address: String,

    /// run client
    #[arg(short, long, default_value_t = true)]
    client: bool,
    /// run server
    #[arg(short, long, default_value_t = true)]
    server: bool,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    if args.server.clone() {
        server(args.clone());
    }
    if args.client.clone() {
        client(args.clone());
    }
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
    let args = Args::parse();

    let (engine_tx, engine_rx) = crossbeam_channel::unbounded::<Event>();
    let (fs_tx, fs_rx) = crossbeam_channel::unbounded::<Event>();

    thread::spawn({
        let rx_address = args.rx_address.clone();
        let tx_address = args.tx_address.clone();
        move || {
            log::info!("Starting Backend Server...");

            let mut txs = HashMap::new();
            txs.insert(events::EventType::FileEvent, fs_tx.clone());
            txs.insert(events::EventType::EngineEvent, engine_tx.clone());
            // Add other event types and their corresponding senders as needed.

            let server = qstudio_tcp::Server::new(rx_address, tx_address);
            server.listen::<EventType, Event, EventResponse>(txs);
        }
    });

    thread::spawn({
        move || {
            log::info!("Starting Engine...");
            let mut engines: HashMap<String, Engine> = HashMap::new();
            let client = qstudio_tcp::Client::new(args.tx_address.clone());
            loop {
                match engine_rx.recv() {
                    Ok(event) => {
                        log::info!("Engine received event: {}", event);
                        // Process the event as needed

                        match event {
                            Event::EngineEvent(engine_event) => {
                                let notification = handle_engine_event(engine_event, &mut engines);
                                match client.send(Copper::ToServer {
                                    client_id: 0,
                                    payload: Event::NotificationEvent(notification),
                                }) {
                                    Ok(_) => log::info!("Engine event sent successfully"),
                                    Err(e) => log::error!("Error sending engine event: {}", e),
                                }
                            }
                            _ => {
                                log::warn!("Engine received unsupported event type");
                            }
                        }
                        // match client.send(Copper::ToServer {
                        //     client_id: 0,
                        //     payload: Event::NotificationEvent(NotificationEvent {
                        //         kind: NotificationKind::Info,
                        //         message: format!("Engine processed event: {}", event),
                        //     }),
                        // }) {
                        //     Ok(_) => log::info!("Engine event sent successfully"),
                        //     Err(e) => log::error!("Error sending engine event: {}", e),
                        // }
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
