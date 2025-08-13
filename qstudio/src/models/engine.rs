use super::notification::Notification;
use crate::Senders;
use engine::controllers::Output;
use engine::Engine;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum EngineEvent {
    Start(String),
    Stop(String),
    Restart(String),
    UpdateSource(String),
    Delete(String),
    None
}

impl EngineEvent {
    pub async fn gate(
        self,
        engine: Arc<Mutex<HashMap<String, Arc<Mutex<Engine>>>>>,
        dataframes: Arc<Mutex<HashMap<String, Arc<Output>>>>,
        channels: Arc<Senders>,
    ) {
        match self {
            EngineEvent::Delete(file_path) => {
                if let Some(_) = engine.lock().unwrap().remove(&file_path) {
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
                if let Some(engine) = engine.lock().unwrap().get(&file_path) {
                    let mut engine = engine.lock().unwrap();
                    match engine.analyze() {
                        Ok(_) => {
                            if let Ok(out) = engine.run().await {
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
                        }
                        Err(e) => {
                            channels
                                .notification_tx
                                .lock()
                                .unwrap()
                                .send(Notification::Error(format!(
                                    "Failed to start engine: {}",
                                    e
                                )))
                                .unwrap();
                        }
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

            EngineEvent::Stop(file_path) => {
                if let Some(engine) = engine.lock().unwrap().get(&file_path) {
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
                if let Some(engine) = engine.lock().unwrap().get(&file_path) {
                    let mut engine = engine.lock().unwrap();
                    if let Ok(out) = engine.update_code().await {
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
                if let Some(engine) = engine.lock().unwrap().get(&file_path) {
                    let engine = engine.lock().unwrap();
                    if let Err(e) = engine.analyze() {
                        channels
                            .notification_tx
                            .lock()
                            .unwrap()
                            .send(Notification::Error(format!(
                                "Failed to analyze engine: {}",
                                e
                            )))
                            .unwrap();
                    } else {
                        channels
                            .notification_tx
                            .lock()
                            .unwrap()
                            .send(Notification::Success(format!(
                                "Engine restarted for file: {}",
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
            _ => {}
        }
    }
}
