#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(rustdoc::missing_crate_level_docs)]

mod graph;

use eframe::egui;
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};
use egui_commonmark::*;
use egui_extras::{Column, TableBuilder};
use engine::parser::Graph;
use engine::Engine;
use polars::frame::DataFrame;
use polars::prelude::*;
use rand::distr::Alphanumeric;
use rand::Rng;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use engine::controllers::Output;    

fn random_string() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect()
}
fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Q Studio",
        options,
        Box::new(|cc| Ok(Box::<QStudio>::default())),
    )
}

use egui::{Ui, WidgetText};
use egui_dock::{DockArea, DockState, OverlayType, Style, TabAddAlign, TabViewer};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum TabKind {
    Code(String),
    Markdown(String),
}

impl TabKind {
    fn as_str(&self) -> &str {
        match self {
            TabKind::Markdown(s) | TabKind::Code(s) => s,
        }
    }
}

type Tab = TabKind;

struct Code {
    code: String,
    file_path: Option<String>,
    file_name: Option<String>,
}

impl Code {
    fn files_been_edited(&self) -> bool {
        if self.file_path.is_some() && self.file_name.is_some() {
            if let Some(path) = &self.file_path {
                match fs::read_to_string(path) {
                    Ok(file_data) => {
                        fn calculate_checksum<T: Hash>(t: &T) -> u64 {
                            let mut s = DefaultHasher::new();
                            t.hash(&mut s);
                            s.finish()
                        }

                        let code_checksum = calculate_checksum(&self.code);
                        let file_checksum = calculate_checksum(&file_data);

                        code_checksum != file_checksum
                    }
                    Err(_) => false,
                }
            } else {
                false
            }
        } else {
            true
        }
    }
}

struct MyTabViewer {
    data: HashMap<String, Code>,
}

impl MyTabViewer {
    fn new() -> Self {
        let mut hm = HashMap::new();
        hm.insert(
            "0".to_string(),
            Code {
                code: String::new(),
                file_path: None,
                file_name: None,
            },
        );
        MyTabViewer { data: hm }
    }

    fn new_code_tab(&mut self, key: String, code: String) {
        self.data.insert(
            key,
            Code {
                code,
                file_path: None,
                file_name: None,
            },
        );
    }

    fn open_code_tab(&mut self, key: &str) -> Option<&Code> {
        self.data.get(key)
    }

    fn mut_code(&mut self, key: String) -> &mut String {
        &mut self
            .data
            .entry(key)
            .or_insert(Code {
                code: String::new(),
                file_path: None,
                file_name: None,
            })
            .code
    }

    fn save_code(&self, key: &str) -> Result<(), String> {
        if let Some(code) = self.data.get(key) {
            if let Some(path) = &code.file_path {
                fs::write(path, &code.code).map_err(|e| e.to_string())
            } else {
                Err("No file path associated with tab.".to_string())
            }
        } else {
            Err("No code found for key.".to_string())
        }
    }

    fn open_file(&mut self, key: &str, path: &str) -> Result<(), String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.data.insert(
            key.to_string(),
            Code {
                code: content,
                file_path: Some(path.to_string()),
                file_name: Some(
                    PathBuf::from(path)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into(),
                ),
            },
        );
        Ok(())
    }
}

impl TabViewer for MyTabViewer {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        if tab.as_str() == "0" {
            return WidgetText::from("Welcome to Q Studio");
        }
        let s = self
            .data
            .get(tab.as_str())
            .map(|code| code.file_name.clone())
            .unwrap_or_default();

        let edited = self
            .data
            .get(tab.as_str())
            .map(|code| code.files_been_edited())
            .unwrap_or(false);

        let title = if edited {
            format!("{}*", s.unwrap_or_else(|| "Untitled".to_string()))
        } else {
            s.unwrap_or_else(|| "Untitled".to_string())
        };

        WidgetText::from(title)
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        let rows = (ui.available_height() / 18.0).floor() as usize;
        match tab {
            TabKind::Code(key) => {
                CodeEditor::default()
                    .id_source("code editor")
                    .with_rows(rows)
                    .with_fontsize(14.0)
                    .with_theme(if ui.ctx().style().visuals.dark_mode {
                        ColorTheme::GITHUB_DARK
                    } else {
                        ColorTheme::GITHUB_LIGHT
                    })
                    .with_syntax(Syntax::sql())
                    .with_numlines(true)
                    .show(ui, self.mut_code(key.clone()));
            }
            TabKind::Markdown(_) => {
                let text = include_str!("../../readme.md");
                let mut cache = CommonMarkCache::default();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let md = CommonMarkViewer::new();
                    md.show(ui, &mut cache, text);
                });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum QueryResult {
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

pub struct QStudio {
    dock_state: DockState<Tab>,
    tab_viewer: MyTabViewer,
    query_result: QueryResult,
    debug_window: bool,
}

impl Default for QStudio {
    fn default() -> Self {
        let tabs = [TabKind::Markdown("0".to_string())].into_iter().collect();
        QStudio {
            dock_state: DockState::new(tabs),
            tab_viewer: MyTabViewer::new(),
            query_result: QueryResult::default(),
            debug_window: true,
        }
    }
}

impl eframe::App for QStudio {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Handle Command+R (Run Query)
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::R)) {
            self.run_query();
        }

        // Handle Command+S (Save Code)
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            if let Some(tab) = self.dock_state.find_active_focused() {
                if let TabKind::Code(key) = tab.1 {
                    if let Err(e) = self.tab_viewer.save_code(key) {
                        eprintln!("Failed to save code: {}", e);
                    }
                }
            }
        }

        self.left_panel(ctx);
        self.right_panel(ctx);

        if self.debug_window {
            self.debug_window(ctx);
        }
    }
}

impl QStudio {
    fn debug_window(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("Debug Panel")
            .resizable(true)
            .default_width(300.0)
            .min_width(200.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Debug Information");
                    if ui.button("Close").clicked() {
                        self.debug_window = false;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        if let Some(tab) = self.dock_state.find_active_focused() {
                            if let TabKind::Code(key) = tab.1 {
                                if let Some(code) = self.tab_viewer.open_code_tab(key) {
                                    ui.collapsing("Engine Query", |ui| {
                                        match Engine::new(code.code.as_str()) {
                                            Ok(engine) => {
                                                ui.label(format!(
                                                    "Engine Query: {:#?}",
                                                    engine.query()
                                                ));
                                            }
                                            Err(e) => {
                                                ui.label(format!(
                                                    "Engine initialization error: {}",
                                                    e
                                                ));
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        ui.collapsing("Dock State", |ui| {
                            ui.label(format!("{:#?}", self.dock_state));
                        });
                        ui.collapsing("Query Result", |ui| {
                            ui.label(format!("{:#?}", self.query_result));
                        });
                    });
            });
    }

    fn left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("Code Editor")
            .resizable(true)
            .default_width(ctx.screen_rect().width() / 2.0)
            .min_width(ctx.screen_rect().width() / 4.0)
            .show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("New").clicked() {
                            let s: String = random_string();
                            self.dock_state
                                .push_to_focused_leaf(TabKind::Code(s.clone()));
                        }
                        if ui.button("Open").clicked() {
                            let path = rfd::FileDialog::new()
                                .add_filter("Quant Query Files", &["qql"])
                                .add_filter("All files", &["*"])
                                .pick_file();

                            if path.is_none() {
                                return;
                            }
                            let path = path.unwrap().to_string_lossy().into_owned();
                            let s = random_string();
                            if let Err(e) = self.tab_viewer.open_file(&s, &path) {
                                eprintln!("Failed to open file: {}", e);
                                return;
                            }

                            let s: String = random_string();
                            let _ = self.tab_viewer.open_file(&s, &path);
                            self.dock_state.push_to_focused_leaf(TabKind::Code(s));
                        }
                        if ui.button("Save").clicked() {}
                    });

                    ui.menu_button("Tools", |ui| {
                        if ui
                            .button("Run Query")
                            .on_hover_text("Execute the query in the editor")
                            .clicked()
                        {
                            self.run_query();
                        }
                        if ui.button("Debug").clicked() {
                            self.debug_window = !self.debug_window;
                        }
                    });

                    ui.menu_button("Themes", |ui| {
                        if ui.button("Toggle Dark Mode").clicked() {
                            ctx.set_visuals(if ctx.style().visuals.dark_mode {
                                egui::Visuals::light()
                            } else {
                                egui::Visuals::dark()
                            });
                        }
                    });
                });

                let mut style = Style::from_egui(ui.style());
                style.overlay.overlay_type = OverlayType::HighlightedAreas;
                style.buttons.add_tab_align = TabAddAlign::Left;
                style.tab_bar.fill_tab_bar = true;

                DockArea::new(&mut self.dock_state)
                    .style(style)
                    .show_inside(ui, &mut self.tab_viewer);
            });
    }
    fn run_query(&mut self) {
        println!("Running query...");
        println!("{:?}", self.dock_state.find_active_focused());
        if let Some(tab) = self.dock_state.find_active_focused() {
            println!("Running query in tab: {}", tab.1.as_str());
            if let TabKind::Code(key) = tab.1 {
                if let Some(code) = self.tab_viewer.open_code_tab(key) {
                    // Clone the code string so no references escape
                    let code_string = code.code.clone();
                    println!("Running query: {}", code_string);
                    let thread = std::thread::spawn(move || {
                        // If Engine::run is async, you need to use a runtime here
                        let engine = Engine::new(code_string.as_str());
                        match engine {
                            Ok(mut engine) => {
                                // If run() is async, use a runtime to block on it
                                let result = tokio::runtime::Runtime::new()
                                    .unwrap()
                                    .block_on(engine.run());
                                result
                            }
                            Err(e) => Err(format!("Failed to create engine: {}", e)),
                        }
                    });

                    match thread.join().unwrap() {
                        Ok(data) => {
                            self.query_result = match data {
                                Output::DataFrame(df) => QueryResult::Table(df),
                                Output::Graph(graph) => QueryResult::Graph(graph),
                            };
                            // println!("Query executed successfully. {:?}", data);
                            // Handle the DataFrame, e.g., display it
                        }
                        Err(e) => {
                            self.query_result = QueryResult::Error(e);
                            // println!("Error executing query: {}", e);
                        }
                    }
                }
            }
        }
    }

    fn right_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| match &self.query_result {
            QueryResult::Table(df) => {
                if !df.is_empty() {
                    show_dataframe_table(ui, df);
                } else {
                    ui.label("No data available.");
                }
            }
            QueryResult::Graph(df) => {
                ui.label("Query Result: Graph");
                // Placeholder for graph rendering logic
                graph::DrawGraph::new(df.clone()).draw(ui);
            }
            QueryResult::Error(e) => {
                ui.label(format!("Error: {}", e));
            }
            QueryResult::None => {
                ui.label("No query result available.");
            }
        });
    }
}
// Helper to render a DataFrame as an egui_extras TableBuilder
fn show_dataframe_table(ui: &mut egui::Ui, df: &DataFrame) {
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
