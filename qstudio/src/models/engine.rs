use super::notification::Notification;
use crate::Senders;
use engine::controllers::Output;
use engine::Engine;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum EngineEvent {
    Start(String),
    Stop(String),
    Restart(String),
    UpdateSource(String),
    Delete(String),
    None,
}

impl EngineEvent {
    pub fn gate(
        self,
        // engine: Arc<Mutex<HashMap<String, Arc<Mutex<Engine>>>>>,
        engine: &mut HashMap<String, Arc<Mutex<Engine>>>,
        dataframes: Arc<Mutex<HashMap<String, Arc<Output>>>>,
        channels: Arc<Senders>,
    ) {
        match self {
            EngineEvent::Delete(file_path) => {
                if let Some(_) = engine.remove(&file_path) {
                    dataframes.lock().unwrap().remove(&file_path);
                    channels
                        .notification_tx
                        .lock()
                        .unwrap()
                        .send(Notification::Success(format!(
                            "Engine deleted for file: {}",
                            file_path
                        )))
                        .unwrap();
                }
            }
            EngineEvent::Start(file_path) => {
                if let Some(engine) = engine.get(&file_path) {
                    let mut engine = engine.lock().unwrap();
                    let rt: tokio::runtime::Runtime = tokio::runtime::Runtime::new().unwrap();
                    let output = rt.block_on(engine.run());
                    if let Ok(out) = output {
                        dataframes
                            .lock()
                            .unwrap()
                            .insert(file_path.clone(), Arc::new(out));
                    }
                    channels
                        .notification_tx
                        .lock()
                        .unwrap()
                        .send(Notification::Success(format!(
                            "Engine started for file: {}",
                            file_path
                        )))
                        .unwrap();
                } else {
                    channels
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
                if let Some(engine) = engine.get(&file_path) {
                    let _engine = engine.lock().unwrap();
                    // engine.status() = engine::EngineStatus::Stopped;
                    channels
                        .notification_tx
                        .lock()
                        .unwrap()
                        .send(Notification::Success(format!(
                            "Engine stopped for file: {}",
                            file_path
                        )))
                        .unwrap();
                } else {
                    channels
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
                if let Some(engine) = engine.get(&file_path) {
                    let mut engine = engine.lock().unwrap();
                    let rt: tokio::runtime::Runtime = tokio::runtime::Runtime::new().unwrap();
                    let output = rt.block_on(engine.update_code());
                    if let Ok(out) = output {
                        dataframes
                            .lock()
                            .unwrap()
                            .insert(file_path.clone(), Arc::new(out));
                    } else {
                        channels
                            .notification_tx
                            .lock()
                            .unwrap()
                            .send(Notification::Error(format!(
                                "Failed to update engine code: {}",
                                file_path
                            )))
                            .unwrap();
                    }
                } else {
                    channels
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
                if let Some(engine) = engine.get(&file_path) {
                    let mut engine = engine.lock().unwrap();
                    let rt: tokio::runtime::Runtime = tokio::runtime::Runtime::new().unwrap();
                    let output = rt.block_on(engine.restart());
                    if let Ok(out) = output {
                        channels
                            .notification_tx()
                            .send(Notification::Success(format!(
                                "Engine restarted for file: {}",
                                file_path
                            )))
                            .unwrap();
                    }
                }
            }
            _ => {}
        }
    }
}
