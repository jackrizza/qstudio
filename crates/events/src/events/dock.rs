use engine::output::Output;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DockEvent {
    OpenFile { path: String },
    ShowFile { name: String, buffer: String },
    ShowGraph { name: String },
    Error { message: String },
    UpdateOutput { name: String, content: Output },
    ShowTrades { name: String },
}

impl DockEvent {
    pub fn execute(&self) -> Self {
        match self {
            DockEvent::OpenFile { path } => {
                log::info!("Opening file: {}", path);
                // Implement file opening logic here
                let buffer = match std::fs::read_to_string(path) {
                    Ok(content) => content,
                    Err(err) => {
                        log::error!("Failed to open file {}: {}", path, err);
                        return DockEvent::Error {
                            message: format!("Failed to open file: {}", err),
                        };
                    }
                };
                DockEvent::ShowFile {
                    name: path.to_string(),
                    buffer,
                }
            }
            DockEvent::ShowGraph { name } => {
                log::info!("Showing graph for: {}", name);
                // Implement graph showing logic here
                self.clone()
            }
            DockEvent::ShowTrades { name } => {
                log::info!("Showing trades for: {}", name);
                // Implement trades showing logic here
                self.clone()
            }

            DockEvent::ShowFile { buffer, .. } => {
                log::info!("Showing file: {}", buffer);
                // Implement file showing logic here
                self.clone()
            }

            DockEvent::Error { message } => {
                log::error!("Dock error: {}", message);
                self.clone()
            }
            DockEvent::UpdateOutput { name, content } => {
                log::info!("Updating output for {}: {:?}", name, content);
                // Implement output updating logic here
                self.clone()
            }
        }
    }
}
