
use engine::parser::Graph;
use polars::frame::DataFrame;
use egui_extras::{TableBuilder, Column};


#[derive(Debug, Clone, PartialEq)]
pub enum QueryResult {
    Table(DataFrame),
    Graph(Graph),
    Error(String),
    None,
}

impl Default for QueryResult {
    fn default() -> Self {
        QueryResult::None
    }
}

#[derive(Debug)]
pub struct Settings {
    pub dark_mode: bool,
}


// Helper to render a DataFrame as an egui_extras TableBuilder
pub fn show_dataframe_table(ui: &mut egui::Ui, df: &DataFrame) {
    let columns = df.get_columns();
    let ncols = columns.len();
    let nrows = df.height();

    let mut table = TableBuilder::new(ui);

    // Add columns (all resizable, remainder for last)
    for i in 0..ncols {
        if i == ncols - 1 {
            table = table.column(Column::remainder());
        } else {
            table = table.column(Column::auto().resizable(true));
        }
    }

    // Header
    table
        .striped(true)
        .header(20.0, |mut header| {
            for col in columns {
                header.col(|ui| {
                    ui.heading(col.name().as_str());
                });
            }
        })
        .body(|mut body| {
            for row_idx in 0..nrows {
                body.row(24.0, |mut row| {
                    for col in columns {
                        row.col(|ui| {
                            let val = match col.get(row_idx) {
                                Ok(v) => v.to_string(),
                                _ => "NULL".into(),
                            };
                            ui.label(format!("{}", val));
                        });
                    }
                });
            }
        });
}
