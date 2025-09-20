use crate::Event;
use crate::EventResponse;
use crate::EventType;
use serde::{Deserialize, Serialize};

use crossbeam_channel::Sender;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NotificationKind {
    Info,
    Warning,
    Error,
}

impl NotificationKind {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "info" => Ok(NotificationKind::Info),
            "warning" => Ok(NotificationKind::Warning),
            "error" => Ok(NotificationKind::Error),
            _ => Err(format!("Unknown NotificationKind: {}", s)),
        }
    }
    pub fn to_string(&self) -> String {
        match self {
            NotificationKind::Info => "info".into(),
            NotificationKind::Warning => "warning".into(),
            NotificationKind::Error => "error".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NotificationEvent {
    pub kind: NotificationKind,
    pub message: String,
}

impl NotificationEvent {
    pub fn execute(&self, notification_tx: Sender<Event>) -> EventResponse {
        let not = self.clone();
        let _ = notification_tx.send(Event::NotificationEvent(not));
        EventResponse::Notification {
            parent_event_type: EventType::NotificationEvent,
            kind: self.kind.to_string(),
            message: self.message.clone(),
        }
    }
}
