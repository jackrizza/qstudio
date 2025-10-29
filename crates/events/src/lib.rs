use busbar::{MakeT, Response, Unravel};
use engine::output::Output;
use serde::{Deserialize, Serialize};

pub mod events;
use crossbeam_channel::Sender;
use events::dock::DockEvent;
use events::engine::EngineEvent;
use events::files::FileEvent;
use events::notifications::NotificationEvent;

use crate::events::notifications::NotificationKind;

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub enum EventType {
    UiEvent,
    NotificationEvent,
    EngineEvent,
    FileEvent,
    DockEvent,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum UiEvent {
    OpenNewWindow,
    CloseWindow,
    MinimizeWindow,
    MaximizeWindow,
    RestoreWindow,
    SetAlwaysOnTop(bool),
    ToggleFullScreen,
    FocusWindow,
    BlurWindow,
    SetWindowTitle(String),
    SetWindowIcon(String), // Path to icon file
    SetWindowSize { width: u32, height: u32 },
    SetWindowPosition { x: i32, y: i32 },
    GetWindowSize,
    GetWindowPosition,
    OpenDevTools,
    CloseDevTools,
    ToggleDevTools,
    ToggleRightBar,
    NewOutputFromServer { filename: String, output: Output },
    ShowGraph { name: String },
    ShowTrades { name: String },
    ShowTables { name: String },
}

impl std::fmt::Debug for UiEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiEvent::OpenNewWindow => f.write_str("UiEvent::OpenNewWindow"),
            UiEvent::CloseWindow => f.write_str("UiEvent::CloseWindow"),
            UiEvent::MinimizeWindow => f.write_str("UiEvent::MinimizeWindow"),
            UiEvent::MaximizeWindow => f.write_str("UiEvent::MaximizeWindow"),
            UiEvent::RestoreWindow => f.write_str("UiEvent::RestoreWindow"),
            UiEvent::SetAlwaysOnTop(top) => write!(f, "UiEvent::SetAlwaysOnTop({})", top),
            UiEvent::ToggleFullScreen => f.write_str("UiEvent::ToggleFullScreen"),
            UiEvent::FocusWindow => f.write_str("UiEvent::FocusWindow"),
            UiEvent::BlurWindow => f.write_str("UiEvent::BlurWindow"),
            UiEvent::SetWindowTitle(title) => write!(f, "UiEvent::SetWindowTitle({})", title),
            UiEvent::SetWindowIcon(path) => write!(f, "UiEvent::SetWindowIcon({})", path),
            UiEvent::SetWindowSize { width, height } => {
                write!(
                    f,
                    "UiEvent::SetWindowSize {{ width: {}, height: {} }}",
                    width, height
                )
            }
            UiEvent::SetWindowPosition { x, y } => {
                write!(f, "UiEvent::SetWindowPosition {{ x: {}, y: {} }}", x, y)
            }
            UiEvent::GetWindowSize => f.write_str("UiEvent::GetWindowSize"),
            UiEvent::GetWindowPosition => f.write_str("UiEvent::GetWindowPosition"),
            UiEvent::OpenDevTools => f.write_str("UiEvent::OpenDevTools"),
            UiEvent::CloseDevTools => f.write_str("UiEvent::CloseDevTools"),
            UiEvent::ToggleDevTools => f.write_str("UiEvent::ToggleDevTools"),
            UiEvent::ToggleRightBar => f.write_str("UiEvent::ToggleRightBar"),
            UiEvent::NewOutputFromServer { filename, .. } => {
                write!(
                    f,
                    "UiEvent::NewOutputFromServer {{ filename: {} }}",
                    filename
                )
            }
            UiEvent::ShowGraph { name } => write!(f, "UiEvent::ShowGraph {{ name: {} }}", name),
            UiEvent::ShowTrades { name } => write!(f, "UiEvent::ShowTrades {{ name: {} }}", name),
            UiEvent::ShowTables { name } => write!(f, "UiEvent::ShowTables {{ name: {} }}", name),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Event {
    Connection { client_id: u32, message: String },

    UiEvent(UiEvent),
    NotificationEvent(NotificationEvent),
    EngineEvent(EngineEvent),
    FileEvent(FileEvent),
    DockEvent(DockEvent),
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
    FileEvent(FileEvent),
    DockEvent(DockEvent),
    EngineEvent(EngineEvent),
    UiEvent(UiEvent),
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
            Event::UiEvent(_) => EventType::UiEvent,
            Event::NotificationEvent(_) => EventType::NotificationEvent,
            Event::EngineEvent(_) => EventType::EngineEvent,
            Event::FileEvent(_) => EventType::FileEvent,
            Event::Connection { .. } => EventType::NotificationEvent, // Example mapping
            Event::DockEvent(_) => EventType::DockEvent,
        }
    }
}

impl Unravel<EventType, Event, EventResponse> for Event {
    fn get_type(&self) -> EventType {
        self.event_type()
    }
    fn do_something<C>(&self, engine_tx: Sender<(C, Event)>, client: C) -> EventResponse {
        match self {
            Event::Connection { client_id, message } => {
                EventResponse::Success(format!("Client {}: {}", client_id, message))
            }
            Event::EngineEvent(engine_event) => engine_event.execute(engine_tx, client),
            Event::NotificationEvent(notification_event) => {
                notification_event.execute(engine_tx, client)
            }
            Event::FileEvent(file_event) => EventResponse::FileEvent(file_event.clone()),
            Event::DockEvent(dock_event) => EventResponse::DockEvent(dock_event.clone()),
            Event::UiEvent(ui_event) => EventResponse::UiEvent(ui_event.clone()),
        }
    }
}

impl Response<EventType, EventResponse> for EventResponse {
    fn event_type(&self) -> EventType {
        match self {
            EventResponse::Success(_) => EventType::NotificationEvent,
            EventResponse::Error(_) => EventType::NotificationEvent,
            EventResponse::Info(_) => EventType::NotificationEvent,
            EventResponse::UiEvent(_) => EventType::UiEvent,
            EventResponse::Notification {
                parent_event_type, ..
            } => parent_event_type.clone(),
            EventResponse::FileEvent(_) => EventType::FileEvent,
            EventResponse::DockEvent(_) => EventType::DockEvent,
            EventResponse::EngineEvent(_) => EventType::EngineEvent,
        }
    }

    fn message(&self) -> String {
        match self {
            EventResponse::Success(msg) => msg.clone(),
            EventResponse::Error(msg) => msg.clone(),
            EventResponse::Info(msg) => msg.clone(),
            EventResponse::Notification { message, .. } => message.clone(),
            EventResponse::FileEvent(_) => "File event".into(),
            EventResponse::DockEvent(_) => "Dock event".into(),
            EventResponse::EngineEvent(_) => "Engine event".into(),
            EventResponse::UiEvent(_) => "UI event".into(),
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
            EventResponse::Notification { kind, message, .. } => {
                Event::NotificationEvent(NotificationEvent {
                    kind: NotificationKind::from_str(kind).unwrap_or(NotificationKind::Info),
                    message: message.clone(),
                })
            }
            EventResponse::FileEvent(file_event) => Event::FileEvent(file_event.clone()),
            EventResponse::DockEvent(dock_event) => Event::DockEvent(dock_event.clone()),
            EventResponse::EngineEvent(engine_event) => Event::EngineEvent(engine_event.clone()),
            EventResponse::UiEvent(ui_event) => Event::UiEvent(ui_event.clone()),
        }
    }
}
