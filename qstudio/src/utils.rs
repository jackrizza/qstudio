use crate::views::dock::PaneType;

pub fn match_file_extension_for_pane_type(
    pane_type: &crate::models::ui::UIEventPane,
    _file_name: &str,
) -> PaneType {
    match pane_type {
        crate::models::ui::UIEventPane::TradeView(file_name) => {
            PaneType::TradeView(file_name.into())
        }

        crate::models::ui::UIEventPane::FlowCharView(file_name) => {
            PaneType::FlowCharView(file_name.into())
        }

        crate::models::ui::UIEventPane::TableView(file_name) => {
            PaneType::TableView(file_name.into())
        }

        crate::models::ui::UIEventPane::GraphView(file_name) => {
            PaneType::GraphView(file_name.into())
        }
        crate::models::ui::UIEventPane::Text(file_name) => {
            match file_name.split('.').last().unwrap_or("") {
                "md" | "markdown" => PaneType::MarkDown(file_name.into()),
                "txt" | "rs" | "py" | "js" | "java" | "c" | "cpp" | "qql" => {
                    PaneType::CodeEditor(file_name.into())
                }
                _ => PaneType::Blank,
            }
        } // Add other mappings as needed
    }
}
