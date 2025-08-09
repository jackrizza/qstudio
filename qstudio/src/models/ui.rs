use crate::models::notification::Notification;


pub enum UIEventPane {
    GraphView(String),
    Text(String),
}
pub enum UIEvent {
    Update,
    Notification(Notification),
    AddPane(UIEventPane),
}

impl UIEventPane {
    pub fn title(&self) -> &str {
        match self {
            UIEventPane::GraphView(title) => title.split('/').last().unwrap_or("Graph View"),
            UIEventPane::Text(title) => title.split('/').last().unwrap_or("Text Editor"),
        }
    }

    pub fn file_path(&self) -> &str {
        match self {
            UIEventPane::GraphView(path) => path,
            UIEventPane::Text(path) => path,
        }
    }
}