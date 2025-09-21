use crate::Event;
use crate::EventResponse;
use engine::controllers::Output;
use serde::{Deserialize, Serialize};

use crossbeam_channel::Sender;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum EngineEvent {
    Start { filename: String },
    Stop { filename: String },
    Status { code: u32, message: String },
    NewEngineMonitor { name: String, status: String },
    GetOutput { name: String },
    Output { name: String, data: Output },
    SaveFile { filename: String, content: String },
    UpdateCode { filename: String },
}

impl EngineEvent {
    pub fn execute<C>(&self, engine_tx: Sender<(C, Event)>, client: C) -> EventResponse {
        match self {
            EngineEvent::Start { filename } => {
                if filename.split('.').last().unwrap_or("") != "qql" {
                    return EventResponse::Info(
                        "Only .qql files are supported for engine start.".into(),
                    );
                }
                let _ = engine_tx.send((
                    client,
                    Event::EngineEvent(EngineEvent::Start {
                        filename: filename.clone(),
                    }),
                ));
                EventResponse::EngineEvent(EngineEvent::NewEngineMonitor {
                    name: filename.clone(),
                    status: "Started".into(),
                })
            }
            EngineEvent::Stop { filename } => {
                EventResponse::Info(format!("Stopping engine for file: {}", filename))
            }
            EngineEvent::Status { code, message } => EventResponse::Info(format!(
                "Engine status - Code: {}, Message: {}",
                code, message
            )),
            EngineEvent::NewEngineMonitor { name, status } => {
                let _ = engine_tx.send((
                    client,
                    Event::EngineEvent(EngineEvent::NewEngineMonitor {
                        name: name.clone(),
                        status: status.clone(),
                    }),
                ));
                EventResponse::Info(format!("New engine monitor created: {}", name))
            }
            EngineEvent::GetOutput { name } => {
                let _ = engine_tx.send((
                    client,
                    Event::EngineEvent(EngineEvent::GetOutput { name: name.clone() }),
                ));
                EventResponse::Info(format!("Requested output for engine: {}", name))
            }
            EngineEvent::Output { name, data } => EventResponse::EngineEvent(EngineEvent::Output {
                name: name.clone(),
                data: data.clone(),
            }),
            EngineEvent::SaveFile { filename, content } => {
                let _ = engine_tx.send((
                    client,
                    Event::EngineEvent(EngineEvent::SaveFile {
                        filename: filename.clone(),
                        content: content.clone(),
                    }),
                ));
                EventResponse::Info(format!("Requested save for file: {}", filename))
            }
            EngineEvent::UpdateCode { filename } => {
                let _ = engine_tx.send((
                    client,
                    Event::EngineEvent(EngineEvent::UpdateCode {
                        filename: filename.clone(),
                    }),
                ));
                EventResponse::Info(format!("Requested code update for file: {}", filename))
            }
        }
    }
}
