use engine::Engine;
use events::events::{
    engine::EngineEvent,
    notifications::{NotificationEvent, NotificationKind},
};
use std::collections::HashMap;

pub fn handle_engine_event(
    event: EngineEvent,
    engines: &mut HashMap<String, Engine>,
) -> NotificationEvent {
    match event {
        EngineEvent::Start { filename } => {
            if engines.contains_key(&filename) {
                log::warn!("Engine for file {} is already running.", filename);
                NotificationEvent {
                    kind: NotificationKind::Warning,
                    message: format!("Engine for file {} is already running.", filename),
                }
            } else {
                match Engine::new(&filename) {
                    Ok(engine) => {
                        engines.insert(filename.clone(), engine);
                        log::info!("Started engine for file: {}", filename);
                        NotificationEvent {
                            kind: NotificationKind::Info,
                            message: format!("Started engine for file: {}", filename),
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to start engine for file {}: {}", filename, e);
                        NotificationEvent {
                            kind: NotificationKind::Error,
                            message: format!("Failed to start engine for file {}: {}", filename, e),
                        }
                    }
                }
            }
        }
        EngineEvent::Stop { filename } => {
            if engines.remove(&filename).is_some() {
                log::info!("Stopped engine for file: {}", filename);
                NotificationEvent {
                    kind: NotificationKind::Info,
                    message: format!("Stopped engine for file: {}", filename),
                }
            } else {
                log::warn!("No running engine found for file: {}", filename);
                NotificationEvent {
                    kind: NotificationKind::Warning,
                    message: format!("No running engine found for file: {}", filename),
                }
            }
        }

        EngineEvent::Status { code, message } => {
            log::info!("Engine status - Code: {}, Message: {}", code, message);
            NotificationEvent {
                kind: NotificationKind::Info,
                message: format!("Engine status - Code: {}, Message: {}", code, message),
            }
        }
        EngineEvent::Status { code, message } => {
            log::info!("Engine status - Code: {}, Message: {}", code, message);
            NotificationEvent {
                kind: NotificationKind::Info,
                message: format!("Engine status - Code: {}, Message: {}", code, message),
            }
        }
    }
}
