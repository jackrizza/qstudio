use crate::events::engine;
use crate::Event;
use crate::EventResponse;
use serde::{Deserialize, Serialize};

use crossbeam_channel::Sender;

#[derive(Serialize, Deserialize, Debug)]
pub enum EngineEvent {
    Start { filename: String },
    Stop { filename: String },
    Status { code: u32, message: String },
}

impl EngineEvent {
    pub fn execute(&self, engine_tx: Sender<Event>) -> EventResponse {
        match self {
            EngineEvent::Start { filename } => {
                let _ = engine_tx.send(Event::EngineEvent(EngineEvent::Start {
                    filename: filename.clone(),
                }));
                EventResponse::Info(format!("Starting engine for file: {}", filename))
            }
            EngineEvent::Stop { filename } => {
                EventResponse::Info(format!("Stopping engine for file: {}", filename))
            }
            EngineEvent::Status { code, message } => EventResponse::Info(format!(
                "Engine status - Code: {}, Message: {}",
                code, message
            )),
        }
    }
}
