use crate::models::notification::Notification;
use crate::views::searchbar::SearchMode;

#[derive(Debug, Clone)]
pub enum UIEventPane {
    GraphView(String),
    TableView(String),
    TradeView(String),
    FlowCharView(String),
    Text(String),
}

#[derive(Debug, Clone)]
pub enum UIEvent {
    Update,
    Notification(Notification),
    AddPane(UIEventPane),
    RemovePane(String),
    SearchBarMode(SearchMode),
    ToggleSearchBar,
}

impl UIEventPane {
    pub fn title(&self) -> &str {
        match self {
            UIEventPane::GraphView(title)
            | UIEventPane::Text(title)
            | UIEventPane::TableView(title)
            | UIEventPane::FlowCharView(title)
            | UIEventPane::TradeView(title) => title.split('/').last().unwrap_or("Table View"),
        }
    }

    pub fn file_path(&self) -> &str {
        match self {
            UIEventPane::GraphView(path)
            | UIEventPane::Text(path)
            | UIEventPane::TableView(path)
            | UIEventPane::FlowCharView(path)
            | UIEventPane::TradeView(path) => path,
        }
    }
}
