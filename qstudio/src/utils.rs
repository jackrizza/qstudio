use engine::Engine;
use events::{events::engine::EngineEvent, EventResponse};
use std::{collections::HashMap, fs};

pub fn handle_engine_event(
    event: EngineEvent,
    engines: &mut HashMap<String, Engine>,
) -> EventResponse {
    match event {
        EngineEvent::Start { filename } => {
            if let Some(engine) = engines.get_mut(&filename) {
                log::warn!("Engine for file {} is already running.", filename);
                let _ = engine.run();
            } else {
                match Engine::new(&filename, "127.0.0.1:7000", None) {
                    Ok(mut engine) => {
                        let _ = engine.run();
                        engines.insert(filename.clone(), engine);
                        log::info!("Started engine for file: {}", filename);
                    }
                    Err(e) => {
                        log::error!("Failed to start engine for file {}: {}", filename, e);
                        return events::EventResponse::Notification {
                            parent_event_type: events::EventType::EngineEvent,
                            kind: "Error".into(),
                            message: format!("Failed to start engine for file {}: {}", filename, e),
                        };
                    }
                };
            }
            events::EventResponse::EngineEvent(EngineEvent::NewEngineMonitor {
                name: filename,
                status: "Started".into(),
            })
        }
        EngineEvent::Stop { filename } => {
            if engines.remove(&filename).is_some() {
                log::info!("Stopped engine for file: {}", filename);
                events::EventResponse::EngineEvent(EngineEvent::NewEngineMonitor {
                    name: filename,
                    status: "Stopped".into(),
                })
            } else {
                log::warn!("No running engine found for file: {}", filename);
                EventResponse::EngineEvent(EngineEvent::NewEngineMonitor {
                    name: filename,
                    status: "Not Found".into(),
                })
            }
        }

        EngineEvent::Status { code, message } => {
            log::info!("Engine status - Code: {}, Message: {}", code, message);
            EventResponse::Notification {
                parent_event_type: events::EventType::EngineEvent,
                kind: "Info".into(),
                message: format!("Engine status - Code: {}, Message: {}", code, message),
            }
        }

        EngineEvent::NewEngineMonitor { name, status } => {
            log::info!(
                "New engine monitor created: {} with status: {}",
                name,
                status
            );
            EventResponse::Info(format!("New engine monitor created: {}", name))
        }

        EngineEvent::GetOutput { name } => {
            if let Some(engine) = engines.get(&name) {
                // Here you would implement logic to fetch and return the engine's output
                log::info!("Fetching output for engine associated with file: {}", name);
                // For demonstration, we'll just return a placeholder response
                EventResponse::EngineEvent(EngineEvent::Output {
                    name: name.clone(),
                    data: engine.get_output().unwrap_or_default(),
                })
            } else {
                log::warn!("No running engine found for file: {}", name);
                EventResponse::Info(format!("No running engine found for file: {}", name))
            }
        }
        EngineEvent::SaveFile { filename, content } => {
            // Here you would implement logic to save the file content
            log::info!("Saving file: {}", filename);
            // For demonstration, we'll just return a placeholder response
            match fs::write(&filename, content) {
                Ok(_) => {
                    log::info!("File saved successfully: {}", filename);
                    handle_engine_event(
                        EngineEvent::UpdateCode {
                            filename: filename.clone(),
                        },
                        engines,
                    )
                }
                Err(e) => {
                    log::error!("Failed to save file {}: {}", filename, e);
                    EventResponse::Notification {
                        parent_event_type: events::EventType::EngineEvent,
                        kind: "Error".into(),
                        message: format!("Failed to save file {}: {}", filename, e),
                    }
                }
            }
        }
        EngineEvent::UpdateCode { filename } => {
            if let Some(engine) = engines.get_mut(&filename) {
                log::info!(
                    "Updating code for engine associated with file: {}",
                    filename
                );
                match engine.update_code() {
                    Ok(_) => {
                        log::info!("Code updated successfully for file: {}", filename);
                        EventResponse::Info(format!(
                            "Code updated successfully for file: {}",
                            filename
                        ))
                    }
                    Err(e) => {
                        log::error!("Failed to update code for file {}: {}", filename, e);
                        EventResponse::Notification {
                            parent_event_type: events::EventType::EngineEvent,
                            kind: "Error".into(),
                            message: format!("Failed to update code for file {}: {}", filename, e),
                        }
                    }
                }
            } else {
                log::warn!("No running engine found for file: {}", filename);
                EventResponse::Info(format!("No running engine found for file: {}", filename))
            }
        }
        _ => {
            log::warn!("Received unsupported EngineEvent: {:?}", event);
            EventResponse::Info("Unsupported EngineEvent".into())
        }
    }
}
