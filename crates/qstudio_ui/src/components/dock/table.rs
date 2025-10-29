use engine::{output::Output, parser::Trades};
use polars::prelude::*;

use crossbeam_channel::Receiver;
use egui::CollapsingHeader;
use egui::RichText;
use egui_extras::{Column as EColumn, TableBuilder};
use polars::prelude::*;
use std::collections::BTreeMap;
use std::collections::HashMap;

/* ------------------------- helpers ------------------------- */

fn is_numeric_dtype(dt: &DataType) -> bool {
    matches!(
        dt,
        DataType::Float64
            | DataType::Float32
            | DataType::Int64
            | DataType::Int32
            | DataType::Int16
            | DataType::Int8
            | DataType::UInt64
            | DataType::UInt32
            | DataType::UInt16
            | DataType::UInt8
    )
}

/// Display-friendly cell text, truncating long strings.
fn fmt_cell(col: &Column, row: usize) -> String {
    match col.get(row) {
        Ok(v) => {
            let s = v.to_string();
            if s.len() > 48 {
                format!("{}…", &s[..48])
            } else {
                s
            }
        }
        _ => "—".to_string(),
    }
}

/* ---------------- per-table state (sort/filter/page) ---------------- */

#[derive(Debug, Default, Clone)]
struct TableState {
    sort_by: Option<(usize, bool)>, // (col_idx, descending)
    filter_text: String,
    filter_col: Option<usize>,
    page_size: usize,
    page: usize,
}

impl TableState {
    fn page_sizes() -> [usize; 5] {
        [25, 50, 100, 250, 500]
    }
}

/* ----------------------------- UI ----------------------------- */

#[derive(Debug, Clone)]
pub struct DrawTablesUi {
    pub output: Option<Output>,
    pub listen: Receiver<Output>,
    states: HashMap<String, TableState>,
}

impl DrawTablesUi {
    pub fn new(listen: Receiver<Output>) -> Self {
        Self {
            output: None,
            listen,
            states: HashMap::new(),
        }
    }

    fn pump_snapshots(&mut self, ctx: &egui::Context) {
        let mut update = false;
        while let Ok(new_output) = self.listen.try_recv() {
            self.output = Some(new_output);
            update = true;
        }
        if update {
            ctx.request_repaint();
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.pump_snapshots(ui.ctx());

        let Some(output) = &self.output else {
            ui.label("No output data available.");
            return;
        };

        let (tables, trades_opt) = match output {
            Output::Data { tables, trades, .. } => (tables, trades.as_ref()),
            _ => {
                ui.label("No table data available.");
                return;
            }
        };

        // stable iteration over table names
        let mut keys: Vec<&String> = tables.keys().collect();
        keys.sort_unstable();

        // Render data frames
        for key in keys {
            let df = &tables[key];
            let title = format!("{key}  ·  {} rows × {} cols", df.height(), df.width());
            egui::CollapsingHeader::new(title)
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(6.0);
                    let state = self.states.entry(key.clone()).or_default();
                    toolbar(ui, key, df, state);
                    ui.separator();
                    render_table(ui, df, state);
                });
            ui.add_space(12.0);
        }

        // Optional trades
        if let Some(tr) = trades_opt {
            let tkey = format!("Trades: {}", tr.over_frame);
            let df = &tr.trades_table;
            egui::CollapsingHeader::new(format!(
                "Trades (over: {})  ·  {} rows",
                tr.over_frame,
                df.height()
            ))
            .default_open(false)
            .show(ui, |ui| {
                ui.add_space(6.0);
                let state = self.states.entry(tkey.clone()).or_default();
                toolbar(ui, &tkey, df, state);
                ui.separator();
                render_table(ui, df, state);

                // compact summary
                ui.add_space(8.0);
                ui.separator();
                let sum = &tr.trade_summary;
                egui::Grid::new(format!("trade_summary_grid_{}", tr.over_frame))
                    .num_columns(4)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Total").weak());
                        ui.monospace(sum.total_trades.to_string());
                        ui.label(RichText::new("Win %").weak());
                        ui.monospace(format!("{:.2}", sum.win_rate));
                        ui.label(RichText::new("Avg +/$1k").weak());
                        ui.monospace(format!("{:.4}", sum.avg_win_per_1000));
                        ui.label(RichText::new("Avg -/$1k").weak());
                        ui.monospace(format!("{:.4}", sum.avg_loss_per_1000));
                        ui.end_row();
                    });
            });
        }
    }
}

/* ------------------------- free helpers (no &mut self) ------------------------- */

fn toolbar(ui: &mut egui::Ui, table_key: &str, df: &DataFrame, state: &mut TableState) {
    ui.horizontal_wrapped(|ui| {
        // Filter column chooser
        let cols = df.get_column_names();
        let mut current_idx = state.filter_col.unwrap_or(0);
        egui::ComboBox::from_id_source(format!("filter_col_{table_key}"))
            .width(180.0)
            .selected_text(cols.get(current_idx).map(|s| s.as_str()).unwrap_or("<col>"))
            .show_ui(ui, |ui| {
                for (i, name) in cols.iter().enumerate() {
                    ui.selectable_value(&mut current_idx, i, name.as_str());
                }
            });
        state.filter_col = Some(current_idx.min(cols.len().saturating_sub(1)));

        // Filter text
        let hint = "filter (substring, case-insensitive)";
        let text_edit = egui::TextEdit::singleline(&mut state.filter_text)
            .hint_text(hint)
            .desired_width(220.0);
        ui.add(text_edit);

        ui.separator();

        // Page size
        let mut ps = if state.page_size == 0 {
            50
        } else {
            state.page_size
        };
        egui::ComboBox::from_id_source(format!("page_size_{table_key}"))
            .width(90.0)
            .selected_text(format!("{} / page", ps))
            .show_ui(ui, |ui| {
                for n in TableState::page_sizes() {
                    ui.selectable_value(&mut ps, n, format!("{n} / page"));
                }
            });
        if ps != state.page_size {
            state.page_size = ps;
            state.page = 0;
        }

        // Paging controls
        let total_rows = df.height();
        let page_sz = state.page_size.max(1);
        let max_page = total_rows.saturating_sub(1) / page_sz;
        let mut page = state.page.min(max_page);

        ui.add_enabled_ui(page > 0, |ui| {
            if ui.button("⟨⟨").clicked() {
                page = 0;
            }
            if ui.button("⟨ Prev").clicked() {
                page = page.saturating_sub(1);
            }
        });
        ui.label(format!("Page {} / {}", page + 1, max_page + 1));
        ui.add_enabled_ui(page < max_page, |ui| {
            if ui.button("Next ⟩").clicked() {
                page = (page + 1).min(max_page);
            }
            if ui.button("⟩⟩").clicked() {
                page = max_page;
            }
        });
        state.page = page;

        ui.separator();

        // Reset sort/filter
        if ui.button("Reset").clicked() {
            state.sort_by = None;
            state.filter_text.clear();
            state.page = 0;
        }
    });
}

fn render_table(ui: &mut egui::Ui, df_in: &DataFrame, state: &mut TableState) {
    // 1) Start with an owned copy we can transform
    let mut df = df_in.clone();

    // 2) Filter (substring on one column, case-insensitive)
    if let (Some(col_idx), true) = (state.filter_col, !state.filter_text.trim().is_empty()) {
        if let Some(colname) = df.get_column_names().get(col_idx) {
            let needle = state.filter_text.to_lowercase();

            // Build a Series view we can index
            if let Ok(series_col) = df.column(colname) {
                let s: Series = series_col.as_series().unwrap().clone();

                // Make a Boolean mask by stringifying each value (works for all dtypes)
                let mask = BooleanChunked::from_iter((0..df.height()).map(|i| {
                    let hit = s
                        .get(i)
                        .map(|v| v.to_string().to_lowercase().contains(&needle))
                        .unwrap_or(false);
                    Some(hit)
                }));

                if let Ok(filtered) = df.filter(&mask) {
                    df = filtered;
                    state.page = 0; // reset paging after filter
                }
            }
        }
    }

    // 3) Sort (single column via Series::arg_sort -> DataFrame::take)
    if let Some((col_idx, desc)) = state.sort_by {
        if let Some(colname) = df.get_column_names().get(col_idx) {
            if let Ok(series_col) = df.column(colname) {
                let s: Series = series_col.as_series().unwrap().clone();

                // Polars 0.49: arg_sort returns UInt32Chunked (not a Result)
                let idx = s.arg_sort(polars::prelude::SortOptions {
                    descending: desc,
                    nulls_last: true,
                    multithreaded: true,
                    maintain_order: false,
                    // required in 0.49:
                    limit: None,
                });

                if let Ok(sorted) = df.take(&idx) {
                    df = sorted;
                }
            }
        }
    }

    // 4) Paging window
    let total_rows = df.height();
    let page_sz = state.page_size.max(1);
    let start = state.page.saturating_mul(page_sz).min(total_rows);
    let end = (start + page_sz).min(total_rows);
    let view_rows = end.saturating_sub(start);

    // 5) Snapshot columns
    let columns = df.get_columns().to_vec();
    let ncols = columns.len();
    let numerics: Vec<bool> = columns
        .iter()
        .map(|c| is_numeric_dtype(c.dtype()))
        .collect();

    // 6) Draw table
    let row_height = 22.0;
    let header_height = 26.0;

    let mut table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .sense(egui::Sense::click())
        .auto_shrink([false, false])
        .column(EColumn::initial(180.0).at_least(80.0)); // first col width

    for _ in 1..ncols {
        table = table.column(EColumn::remainder().at_least(80.0));
    }

    table
        .header(header_height, |mut header| {
            for (i, col) in columns.iter().enumerate() {
                let is_sorted = state.sort_by.map(|(idx, _)| idx == i).unwrap_or(false);
                let arrow = state
                    .sort_by
                    .and_then(|(idx, desc)| (idx == i).then_some(desc))
                    .map(|d| if d { " ⬇" } else { " ⬆" })
                    .unwrap_or("");
                header.col(|ui| {
                    let lbl = format!("{}{}", col.name(), arrow);
                    if ui
                        .selectable_label(is_sorted, RichText::new(lbl).strong())
                        .clicked()
                    {
                        state.sort_by = match state.sort_by {
                            Some((idx, desc)) if idx == i => Some((i, !desc)),
                            _ => Some((i, false)),
                        };
                    }
                });
            }
        })
        .body(|mut body| {
            body.rows(row_height, view_rows, |mut row| {
                let df_row = start + row.index();
                for (col_idx, col) in columns.iter().enumerate() {
                    row.col(|ui| {
                        let txt = fmt_cell(col, df_row);
                        let mono = RichText::new(txt).monospace();
                        if numerics[col_idx] {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(mono);
                                },
                            );
                        } else {
                            ui.label(mono);
                        }
                    });
                }
            });
        });
}

/* ---------------- Optional: legacy helpers removed ----------------
   The old egui_table-based DataFrameTable/_show_* functions were using a
   different crate (`egui_table`). Since we migrated to `egui_extras::TableBuilder`,
   those are no longer needed and have been deleted to avoid mixed APIs.
   If you still need them elsewhere, keep them in a separate module/file.
------------------------------------------------------------------- */

pub struct DataFrameTable {
    df: DataFrame,
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

pub fn _show_dataframe_table(ui: &mut egui::Ui, df: &DataFrame) {
    let ncols = df.get_columns().len();
    let nrows = df.height() as u64;

    let table = egui_table::Table::new()
        .num_rows(nrows)
        .columns(vec![egui_table::Column::new(100.0).resizable(true); ncols])
        .headers([egui_table::HeaderRow::new(24.0)])
        .auto_size_mode(egui_table::AutoSizeMode::default());

    let mut delegate = DataFrameTable { df: df.clone() };
    table.show(ui, &mut delegate);
}

pub fn _show_trades_table(ui: &mut egui::Ui, trades: &Trades) {
    let df = &trades.trades_table;
    let ncols = df.get_columns().len();
    let nrows = df.height() as u64;

    let table = egui_table::Table::new()
        .num_rows(nrows)
        .columns(vec![egui_table::Column::new(100.0).resizable(true); ncols])
        .headers([egui_table::HeaderRow::new(24.0)])
        .auto_size_mode(egui_table::AutoSizeMode::default());

    let mut delegate = DataFrameTable { df: df.clone() };
    table.show(ui, &mut delegate);
}
