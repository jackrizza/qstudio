use crate::models::engine::EngineEvent;
use crate::Channels;
use egui::Ui;
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState, TabIndex, TabViewer};
use engine::controllers::Output;
use polars::frame::DataFrame;
use std::collections::HashMap;
use std::f32::consts::E;
use std::fs;
use std::sync::{Arc, Mutex};

// Add these imports for CommonMark markdown rendering
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

// Import Notification type

mod editor;
mod graph;
mod markdown;

// Use PaneType as your Tab data
#[derive(Debug, Clone)]
pub enum PaneType {
    MarkDown(String),
    Blank,
    CodeEditor(String),
    GraphView(String),
}

impl PaneType {
    pub fn title(&self) -> String {
        match self {
            PaneType::MarkDown(title) => title.clone(),
            PaneType::Blank => "Untitled".to_string(),
            PaneType::CodeEditor(title) => title.clone(),
            PaneType::GraphView(title) => format!("Graph -  {}", title.clone()),
        }
    }
}

pub struct MyTabViewer {
    pub dataframes: Arc<Mutex<HashMap<String, Arc<Output>>>>, // the will be a dataframe later
    pub buffers: HashMap<String, String>,
    pub channels: Arc<Channels>,
}

impl MyTabViewer {
    pub fn new(
        dataframes: Arc<Mutex<HashMap<String, Arc<Output>>>>,
        channels: Arc<Channels>,
    ) -> Self {
        MyTabViewer {
            dataframes,
            buffers: HashMap::new(),
            channels,
        }
    }

    pub fn create_buffer(&mut self, file_name: String) {
        let buffer = fs::read_to_string(&file_name).unwrap_or_else(|_| String::new());
        self.buffers.insert(file_name, buffer);
    }

    pub fn get_mut_buffer(&mut self, file_name: &str) -> &mut String {
        self.buffers
            .entry(file_name.to_string())
            .or_insert_with(String::new)
    }
}

impl TabViewer for MyTabViewer {
    type Tab = PaneType;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn on_close(&mut self, _tab: &mut Self::Tab) -> OnCloseResponse {
        let _ = self
            .channels
            .engine_tx
            .lock()
            .unwrap()
            .send(EngineEvent::Delete(_tab.title()));

        let _ = self
            .channels
            .ui_tx
            .lock()
            .unwrap()
            .send(crate::models::ui::UIEvent::RemovePane(
                _tab.title().to_string(),
            ));

        OnCloseResponse::Close
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            PaneType::MarkDown(content) => {
                let hello_world = include_str!("../../../../qql.md");
                let text = if content == "Hello World" {
                    hello_world.to_string()
                } else {
                    fs::read_to_string(content)
                        .unwrap_or_else(|_| "Failed to read file".to_string())
                };
                let mut cache = CommonMarkCache::default();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let md = CommonMarkViewer::new();
                    md.show(ui, &mut cache, &text);
                });
            }
            PaneType::Blank => {
                ui.label("This is a blank pane.");
            }
            PaneType::CodeEditor(file_name) => {
                let channels = Arc::clone(&self.channels);
                editor::code_editor(
                    ui,
                    file_name.clone(),
                    self.get_mut_buffer(file_name),
                    channels,
                );
            }
            PaneType::GraphView(file_name) => {
                let dataframes = Arc::clone(&self.dataframes);
                if let Some(out) = dataframes.lock().unwrap().get(file_name) {
                    if let Some(graph) = out.get_graph() {
                        graph::DrawGraph::new(graph.clone()).draw(ui);
                    } else {
                        ui.label("No graph data available.");
                    }
                };
            }
        }
    }
}
#[derive(Debug, Clone)]
pub struct PaneDock {
    dock_state: DockState<PaneType>,
}

impl PaneDock {
    pub fn new() -> Self {
        let tabs = vec![PaneType::MarkDown("Hello World".into())];
        PaneDock {
            dock_state: DockState::new(tabs),
        }
    }

    pub fn remove_pane(&mut self, title: &str) {
        // Iterate through all tabs and remove the one with the matching title
        let mut to_remove = None;
        for ((surface, node), tab_index, tab) in self.dock_state.iter_all_tabs().enumerate().map(|(i, ((surface, node), tab))| ((surface, node), TabIndex(i), tab)) {
            if tab.title() == title {
                to_remove = Some((surface, node, tab_index));
                break;
            }
        }
        if let Some((surface, node, tab_index)) = to_remove {
            self.dock_state.remove_tab((surface, node, tab_index));
        }
    }

    pub fn add_pane(&mut self, pane: PaneType, tab_viewer: &mut MyTabViewer) {
        // Try to find an existing matching tab
        tab_viewer.create_buffer(pane.title().to_string());
        let existing = self
            .dock_state
            .iter_all_tabs() // -> ((SurfaceIndex, NodeIndex), &Tab)
            .enumerate()
            .find_map(|(tab_index, ((surface, node), tab))| {
                let same = match (tab, &pane) {
                    (PaneType::CodeEditor(t), PaneType::CodeEditor(p)) => t == p,
                    (PaneType::MarkDown(t), PaneType::MarkDown(p)) => t == p,
                    (PaneType::Blank, PaneType::Blank) => true,
                    (PaneType::GraphView(t), PaneType::GraphView(p)) => t == p,
                    _ => false,
                };
                same.then_some((surface, node, TabIndex(tab_index)))
            });

        if let Some((surface, node, tab_idx)) = existing {
            // Focus and activate the already open tab
            self.dock_state
                .set_focused_node_and_surface((surface, node)); // focus leaf
            self.dock_state.set_active_tab((surface, node, tab_idx)); // make it active
                                                                      // Methods used here: iter_all_tabs, set_focused_node_and_surface, set_active_tab.
                                                                      // See docs for these APIs.  :contentReference[oaicite:1]{index=1}
        } else {
            // Add new tab to the focused leaf (or first leaf / create leaf)
            self.dock_state.push_to_focused_leaf(pane); // behavior described in docs. :contentReference[oaicite:2]{index=2}
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, tab_viewer: &mut MyTabViewer) {
        DockArea::new(&mut self.dock_state)
            .show_add_buttons(false)
            .show_close_buttons(true)
            .show_leaf_close_all_buttons(false)
            .show_leaf_collapse_buttons(false)
            .show_inside(ui, tab_viewer);
    }
}
