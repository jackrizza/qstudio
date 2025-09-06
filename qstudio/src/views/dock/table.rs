use engine::parser::Trades;
use polars::lazy::dsl::col;
use polars::prelude::*;

pub struct DataFrameTable {
    df: DataFrame,
    default_column: egui_table::Column,
    auto_size_mode: egui_table::AutoSizeMode,
    top_row_height: f32,
    row_height: f32,
}

impl DataFrameTable {
    pub fn new(df: DataFrame) -> Self {
        let ncols = df.get_columns().len();
        Self {
            df,
            default_column: egui_table::Column::new(100.0)
                .range(10.0..=500.0)
                .resizable(true),
            auto_size_mode: egui_table::AutoSizeMode::default(),
            top_row_height: 24.0,
            row_height: 18.0,
        }
    }
}

impl egui_table::TableDelegate for DataFrameTable {
    fn cell_ui(&mut self, ui: &mut egui::Ui, cell_info: &egui_table::CellInfo) {
        let egui_table::CellInfo { row_nr, col_nr, .. } = *cell_info;
        let columns = self.df.get_columns();
        if let Some(col) = columns.get(col_nr) {
            let val = match col.get(row_nr as usize) {
                Ok(v) => v.to_string(),
                _ => "NULL".into(),
            };
            ui.label(val);
        }
    }

    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell_info: &egui_table::HeaderCellInfo) {
        let columns = self.df.get_columns();
        if let Some(col) = columns.get(cell_info.col_range.start) {
            ui.heading(col.name().to_string());
        }
    }
}

pub fn show_dataframe_table(ui: &mut egui::Ui, df: &DataFrame) {
    let ncols = df.get_columns().len();
    let nrows = df.height() as u64;

    let mut table = egui_table::Table::new()
        .num_rows(nrows)
        .columns(vec![egui_table::Column::new(100.0).resizable(true); ncols])
        .headers([egui_table::HeaderRow::new(24.0)])
        .auto_size_mode(egui_table::AutoSizeMode::default());

    let mut delegate = DataFrameTable::new(df.clone());
    table.show(ui, &mut delegate);
}

pub fn show_trades_table(ui: &mut egui::Ui, trades: &Trades) {
    let df = &trades.trades_table;
    let ncols = df.get_columns().len();
    let nrows = df.height() as u64;

    let mut table = egui_table::Table::new()
        .num_rows(nrows)
        .columns(vec![egui_table::Column::new(100.0).resizable(true); ncols])
        .headers([egui_table::HeaderRow::new(24.0)])
        .auto_size_mode(egui_table::AutoSizeMode::default());

    let mut delegate = DataFrameTable::new(df.clone());
    table.show(ui, &mut delegate);
}
