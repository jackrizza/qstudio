use egui_notify::Toasts;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Notification {
    Success(String),
    Info(String),
    Warning(String),
    Error(String),
}

impl Notification {
    pub fn create_toast(&self, notification: &mut Toasts) {
        match self {
            Notification::Success(msg) => {
                notification
                    .success(msg)
                    .duration(Some(Duration::from_secs(3)));
            }
            Notification::Info(msg) => {
                notification
                    .info(msg)
                    .duration(Some(Duration::from_secs(3)));
            }
            Notification::Warning(msg) => {
                notification
                    .warning(msg)
                    .duration(Some(Duration::from_secs(3)));
            }
            Notification::Error(msg) => {
                notification
                    .error(msg)
                    .duration(Some(Duration::from_secs(3)));
            }
        };
    }
}
