use crate::graph::DrawGraph;
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState, NodeIndex, Style};
use egui_extras::{Column, TableBuilder};
use polars::frame::DataFrame;
// Add import for Graph type
use engine::parser::Graph;
use std::collections::HashMap;

struct PreviewTabViewer {
    graph: Option<Graph>,
    tables: HashMap<String, DataFrame>,
    trades: Option<DataFrame>,
}

impl egui_dock::TabViewer for PreviewTabViewer {
    type Tab = String;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        (&*tab).into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab.as_str() {
            "Preview" => match self.graph {
                Some(ref graph) => {
                    DrawGraph::new(graph.clone()).draw(ui);
                }
                None => {
                    ui.label("No graph data available.");
                }
            },
            "Trades" => {
                trade_panel(ui);
            }
            _ => {
                ui.label(format!("Preview for tab: {}", tab));
            }
        }
    }

    fn on_close(&mut self, _tab: &mut Self::Tab) -> OnCloseResponse {
        println!("Closed tab: {_tab}");
        OnCloseResponse::Close
    }
}

pub struct Preview {
    pub tree: DockState<String>,
}

impl Default for Preview {
    fn default() -> Self {
        let mut tree = DockState::new(vec!["Preview".to_owned()]);

        // You can modify the tree before constructing the dock
        let [a, b] =
            tree.main_surface_mut()
                .split_below(NodeIndex::root(), 0.75, vec!["Trades".to_owned()]);

        Self { tree }
    }
}

impl Preview {
    pub fn add_table(&mut self, name: String) {
        self.tree.main_surface_mut().push_to_focused_leaf(name);
    }
}

impl Preview {
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        graph: Option<Graph>,
        tables: HashMap<String, DataFrame>,
        trades: Option<DataFrame>,
    ) {
        DockArea::new(&mut self.tree)
            .show_close_buttons(false)
            .show_leaf_close_all_buttons(false)
            .style(Style::from_egui(ctx.style().as_ref()))
            .show(
                ctx,
                &mut PreviewTabViewer {
                    graph,
                    tables,
                    trades,
                },
            );
    }

    pub fn error(&mut self, ctx: &egui::Context, error: String) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!("Error: {}", error));
        });
    }
}

fn trade_panel(ui: &mut egui::Ui) {
    TableBuilder::new(ui)
        .striped(true)
        .column(Column::initial(150.0).resizable(true))
        .column(Column::initial(175.0).resizable(true))
        .column(Column::initial(150.0).resizable(true))
        .column(Column::initial(150.0).resizable(true))
        .column(Column::initial(150.0).resizable(true))
        .column(Column::initial(150.0).resizable(true))
        .column(Column::initial(100.0).resizable(true))
        .column(Column::initial(125.0).resizable(true))
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.heading("Entrance Date");
            });
            header.col(|ui| {
                ui.heading("Entrance Strike Price");
            });
            header.col(|ui| {
                ui.heading("Exit Date");
            });
            header.col(|ui| {
                ui.heading("Exit Strike Price");
            });
            header.col(|ui| {
                ui.heading("Limit Date");
            });
            header.col(|ui| {
                ui.heading("Limit Strike Price");
            });
            header.col(|ui| {
                ui.heading("Profit");
            });
            header.col(|ui| {
                ui.heading("Days Held");
            });
        })
        .body(|mut body| {
            body.row(30.0, |mut row| {
                row.col(|ui| {
                    ui.label("2023-01-01");
                });
                row.col(|ui| {
                    ui.label("$100");
                });
                row.col(|ui| {
                    ui.label("2023-01-02");
                });
                row.col(|ui| {
                    ui.label("$150");
                });
                row.col(|ui| {
                    ui.label("-");
                });
                row.col(|ui| {
                    ui.label("-");
                });
                row.col(|ui| {
                    ui.label("$50");
                });
                row.col(|ui| {
                    ui.label("1");
                });
            });
        });
}
