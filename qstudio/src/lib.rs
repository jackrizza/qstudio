use core_affinity::CoreId;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

pub mod ff;
pub mod models;
pub mod tick;
pub mod utils;
pub mod views;

use models::engine::EngineEvent;
use models::notification::Notification;
use models::ui::UIEvent;

use engine::controllers::Output;
use engine::Engine;

use crate::ff::AsyncExec;

pub struct Senders {
    pub ui_tx: Arc<Mutex<Sender<UIEvent>>>,
    pub notification_tx: Arc<Mutex<Sender<Notification>>>,
    pub engine_tx: Arc<Mutex<Sender<Mutex<EngineEvent>>>>,
}

impl Senders {
    pub fn ui_tx(&self) -> Sender<UIEvent> {
        self.ui_tx.lock().unwrap().clone()
    }

    pub fn notification_tx(&self) -> Sender<Notification> {
        self.notification_tx.lock().unwrap().clone()
    }

    pub fn engine_tx(&self) -> Sender<Mutex<EngineEvent>> {
        self.engine_tx.lock().unwrap().clone()
    }
}

pub struct Receivers {
    pub ui_rx: Arc<Mutex<Receiver<UIEvent>>>,
    pub notification_rx: Arc<Mutex<Receiver<Notification>>>,
    pub engine_rx: Arc<Mutex<Receiver<Mutex<EngineEvent>>>>,
}

impl Receivers {
    pub fn ui_rx(&self) -> Result<UIEvent, String> {
        let ui_rx = self.ui_rx.lock().unwrap();
        ui_rx
            .try_recv()
            .map_err(|_| "Failed to receive UI event".to_string())
    }

    pub fn notification_rx(&self) -> Result<Notification, String> {
        let notification_rx = self.notification_rx.lock().unwrap();
        notification_rx
            .try_recv()
            .map_err(|_| "Failed to receive Notification event".to_string())
    }

    pub fn engine_rx(&self) -> Result<Mutex<EngineEvent>, String> {
        let engine_rx = self.engine_rx.lock().unwrap();
        engine_rx
            .try_recv()
            .map_err(|_| "Failed to receive Engine event".to_string())
    }

    pub fn ui_recv_blocking(&self) -> Result<UIEvent, std::sync::mpsc::RecvError> {
        self.ui_rx.lock().unwrap().recv()
    }
    pub fn notification_recv_blocking(&self) -> Result<Notification, std::sync::mpsc::RecvError> {
        self.notification_rx.lock().unwrap().recv()
    }
    pub fn engine_recv_blocking(&self) -> Result<Mutex<EngineEvent>, std::sync::mpsc::RecvError> {
        self.engine_rx.lock().unwrap().recv()
    }
}

pub struct Channels {
    pub senders: Arc<Senders>,
    pub receivers: Arc<Receivers>,
}

impl Channels {
    pub fn new() -> Self {
        let (ui_tx, ui_rx) = std::sync::mpsc::channel();
        let (notification_tx, notification_rx) = std::sync::mpsc::channel();
        let (engine_tx, engine_rx) = std::sync::mpsc::channel();

        Channels {
            senders: Arc::new(Senders {
                ui_tx: Arc::new(Mutex::new(ui_tx)),
                notification_tx: Arc::new(Mutex::new(notification_tx)),
                engine_tx: Arc::new(Mutex::new(engine_tx)),
            }),
            receivers: Arc::new(Receivers {
                ui_rx: Arc::new(Mutex::new(ui_rx)),
                notification_rx: Arc::new(Mutex::new(notification_rx)),
                engine_rx: Arc::new(Mutex::new(engine_rx)),
            }),
        }
    }

    pub fn senders(&self) -> Arc<Senders> {
        Arc::clone(&self.senders)
    }

    pub fn receivers(&self) -> Arc<Receivers> {
        Arc::clone(&self.receivers)
    }

    pub fn log_channel_events(&self) {
        // Check each channel non-blocking, then sleep briefly to avoid busy waiting
        if let Ok(event) = self.receivers().ui_rx() {
            println!("UI Event: {:?}", event);
        }
        if let Ok(event) = self.receivers().notification_rx() {
            println!("Notification Event: {:?}", event);
        }
        if let Ok(event) = self.receivers().engine_rx() {
            println!("Engine Event: {:?}", event);
        }
    }

    pub fn notification_thread(&mut self) {
        let ui_tx = Arc::clone(&self.senders.ui_tx);
        let receivers = Arc::clone(&self.receivers);
        thread::spawn(move || {
            log::info!("Waiting for notification events...");
            while let Ok(notification) = receivers.notification_recv_blocking() {
                log::info!("Notification received: {:?}", notification);
                let _ = ui_tx
                    .lock()
                    .unwrap()
                    .send(UIEvent::Notification(notification));
            }
        });
    }

    pub fn engine_thread(
        &mut self,
        engine: Arc<Mutex<std::collections::HashMap<String, Arc<Mutex<Engine>>>>>,
        dataframes: Arc<Mutex<std::collections::HashMap<String, Arc<Output>>>>,
    ) {
        let receivers = Arc::clone(&self.receivers);
        let senders = Arc::clone(&self.senders);

        thread::spawn(move || {
            log::info!("Waiting for engine events...");

            while let Ok(event) = receivers.engine_recv_blocking() {
                log::info!("Engine event received: {:?}", event);
                let engine = Arc::clone(&engine);
                let dataframes = Arc::clone(&dataframes);
                let senders = Arc::clone(&senders);

                let e = event.lock().unwrap().clone();
                let mut engine = engine.lock().unwrap();
                // let mut engine = engine.get_mut(e.file_path()).unwrap().lock().unwrap();
                // the reason why this is blocking is later gate calls .run on engine
                // which is an async function
                // possible fix is to move the engine lock to here
                e.gate(&mut engine, dataframes, senders);
            }
        });
    }
}
