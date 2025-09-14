use busbar::{MakeT, Response, Unravel};
use serde::{Deserialize, Serialize};

pub mod events;
use events::engine::EngineEvent;
use events::notifications::NotificationEvent;

use crossbeam_channel::Sender;

use crate::events::notifications::NotificationKind;

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub enum EventType {
    UiEvent,
    NotificationEvent,
    EngineEvent,
    FileEvent,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Event {
    Connection { client_id: u32, message: String },

    UiEvent,
    NotificationEvent(NotificationEvent),
    EngineEvent(EngineEvent),
    FileEvent,
}

#[derive(Debug)]
pub enum EventResponse {
    Success(String),
    Error(String),
    Info(String),
    Notification {
        parent_event_type: EventType,
        kind: String,
        message: String,
    },
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::fmt::Display for EventResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Event {
    pub fn event_type(&self) -> EventType {
        match self {
            Event::UiEvent => EventType::UiEvent,
            Event::NotificationEvent(_) => EventType::NotificationEvent,
            Event::EngineEvent(_) => EventType::EngineEvent,
            Event::FileEvent => EventType::FileEvent,
            Event::Connection { .. } => EventType::NotificationEvent, // Example mapping
        }
    }
}

impl Unravel<EventType, Event, EventResponse> for Event {
    fn get_type(&self) -> EventType {
        self.event_type()
    }
    fn do_something(&self, engine_tx: Sender<Event>) -> EventResponse {
        match self {
            Event::Connection { client_id, message } => {
                EventResponse::Success(format!("Client {}: {}", client_id, message))
            }
            Event::EngineEvent(engine_event) => engine_event.execute(engine_tx),
            Event::NotificationEvent(notification_event) => {
                notification_event.execute(engine_tx)
            }
            _ => EventResponse::Info("Event processed".into()),
        }
    }
}

impl Response<EventType, EventResponse> for EventResponse {
    fn event_type(&self) -> EventType {
        match self {
            EventResponse::Success(_) => EventType::NotificationEvent,
            EventResponse::Error(_) => EventType::NotificationEvent,
            EventResponse::Info(_) => EventType::NotificationEvent,
            EventResponse::Notification {
                parent_event_type, ..
            } => parent_event_type.clone(),
        }
    }

    fn message(&self) -> String {
        match self {
            EventResponse::Success(msg) => msg.clone(),
            EventResponse::Error(msg) => msg.clone(),
            EventResponse::Info(msg) => msg.clone(),
            EventResponse::Notification { message, .. } => message.clone(),
        }
    }
    fn default() -> Self {
        EventResponse::Info("Default response".into())
    }
}

impl MakeT<Event> for EventResponse {
    fn make_t(&self) -> Event {
        match self {
            EventResponse::Success(msg) => Event::NotificationEvent(NotificationEvent {
                kind: NotificationKind::Info,
                message: msg.clone(),
            }),
            EventResponse::Error(msg) => Event::NotificationEvent(NotificationEvent {
                kind: NotificationKind::Error,
                message: msg.clone(),
            }),
            EventResponse::Info(msg) => Event::NotificationEvent(NotificationEvent {
                kind: NotificationKind::Info,
                message: msg.clone(),
            }),
            EventResponse::Notification {
                parent_event_type,
                kind,
                message,
            } => Event::NotificationEvent(NotificationEvent {
                kind: NotificationKind::from_str(kind).unwrap_or(NotificationKind::Info),
                message: message.clone(),
            }),
        }
    }
}
