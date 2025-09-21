use egui::{CornerRadius, Margin, Ui};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{
    DockArea, DockState, NodeIndex, OverlayType, SeparatorStyle, Style, SurfaceIndex, TabAddAlign,
    TabIndex, TabViewer,
};
use engine::controllers::Output;
use engine::Engine;
use std::sync::Arc;

use busbar::Aluminum;
use events::Event;

use events::events::engine::EngineEvent;

use crossbeam_channel::{Receiver, Sender};
use std::collections::HashMap;

// Add these imports for CommonMark markdown rendering
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

// Import Notification type

mod editor;
mod graph;
mod markdown;
mod table;
mod trade;

// Import TradeSummary type
use engine::utils::trade::TradeSummary;

// Use PaneType as your Tab data
#[derive(Debug, Clone)]
pub enum PaneType {
    MarkDown {
        name: String,
        content: String,
    },
    Blank,
    CodeEditor {
        file_name: String,
        buffer: String,
    },
    GraphView {
        title: String,
        data: Receiver<Output>,
        draw_graph: Option<graph::DrawGraphUi>,
    },
    TableView {
        title: String,
        data: Receiver<Output>,
    },
    TradeView {
        title: String,
        summary: Receiver<Output>,
        trade_summary: Option<trade::TradeSummaryUi>,
    },
    FlowCharView {
        title: String,
        data: Receiver<Output>,
    },
}

impl PaneType {
    pub fn title(&self) -> String {
        match self {
            PaneType::MarkDown { name, .. } => format!("Markdown - {}", name),
            PaneType::Blank => "Blank".to_string(),
            PaneType::CodeEditor { file_name, .. } => format!("Code Editor - {}", file_name),
            PaneType::GraphView { title, .. } => format!("Graph View - {}", title),
            PaneType::TableView { title, .. } => format!("Table View - {}", title),
            PaneType::TradeView { title, .. } => format!("Trade View - {}", title),
            PaneType::FlowCharView { title, .. } => format!("Flow Chart View - {}", title),
        }
    }
}

#[derive(Clone)]
pub struct MyTabViewer {
    pane_aluminum: Arc<Aluminum<events::Event>>,
}

impl MyTabViewer {
    pub fn new(pane_aluminum: Arc<Aluminum<events::Event>>) -> Self {
        MyTabViewer { pane_aluminum }
    }
}

impl TabViewer for MyTabViewer {
    type Tab = PaneType;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn on_close(&mut self, _tab: &mut Self::Tab) -> OnCloseResponse {
        OnCloseResponse::Close
    }

    // ← Add your custom items here
    fn context_menu(
        &mut self,
        ui: &mut Ui,
        tab: &mut Self::Tab,
        _surface: SurfaceIndex,
        _node: NodeIndex,
    ) {
        // Import SearchMode if not already imported
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            PaneType::MarkDown { content, .. } => {
                // Use CommonMarkViewer to render markdown content
                CommonMarkViewer::new().show(ui, &mut CommonMarkCache::default(), content);
            }
            PaneType::Blank => {
                ui.label("This is a blank pane.");
            }
            PaneType::CodeEditor { file_name, buffer } => {
                let ctx = ui.ctx();

                if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
                    // Handle Command+S (save) event here
                    // For example, you could send an EngineEvent or trigger a save function
                    // Example:
                    // engine_event_sender.send(EngineEvent::SaveFile(file_name.clone(), buffer.clone()));
                    log::info!("Saving file: {}", file_name);
                    self.pane_aluminum
                        .frontend_tx
                        .send(Event::EngineEvent(EngineEvent::SaveFile {
                            filename: file_name.clone(),
                            content: buffer.clone(),
                        }))
                        .unwrap_or_else(|e| {
                            log::error!("Failed to send SaveFile event: {}", e);
                        });
                }
                editor::code_editor(ui, file_name.to_string(), buffer);
            }
            PaneType::GraphView {
                title,
                data,
                draw_graph,
            } => {
                if draw_graph.is_none() {
                    *draw_graph = Some(graph::DrawGraphUi::new(data.clone()));
                }
                draw_graph.as_mut().unwrap().ui(ui);
            }
            PaneType::TradeView {
                title,
                summary,
                trade_summary,
            } => {
                if trade_summary.is_none() {
                    *trade_summary = Some(trade::TradeSummaryUi::new(summary.clone()));
                }
                trade_summary.as_mut().unwrap().ui(ui);
            }
            _ => {
                ui.label("Unsupported pane type");
            }
        }
    }
}
#[derive(Debug, Clone)]
pub struct PaneDock {
    dock_state: DockState<PaneType>,
    dock_aluminum: Arc<Aluminum<events::Event>>,
    panel_channels: HashMap<String, Vec<(Sender<Output>, Receiver<Output>)>>,
}

impl PaneDock {
    pub fn new(dock_aluminum: Arc<Aluminum<events::Event>>) -> Self {
        let tabs = vec![PaneType::MarkDown {
            name: "Hello World".into(),
            content: "# Welcome to QStudio\nThis is a markdown pane.".into(),
        }];
        PaneDock {
            dock_state: DockState::new(tabs),
            dock_aluminum,
            panel_channels: HashMap::new(),
        }
    }

    fn pump_snapshots(&mut self, ui: &mut Ui) {
        // Drain all pending messages and keep only the latest Fs snapshot
        let mut updated = false;
        while let Ok(ev) = self.dock_aluminum.dock_rx.try_recv() {
            if let Event::DockEvent(dock_event) = ev {
                match dock_event {
                    events::events::dock::DockEvent::ShowFile { name, buffer } => {
                        self.panel_channels
                            .entry(name.clone())
                            .or_insert_with(Vec::new)
                            .push(crossbeam_channel::unbounded::<Output>());
                        log::info!("Showing file from Dock");
                        match name.split(".").last() {
                            Some("md") | Some("markdown") => {
                                self.dock_state.push_to_focused_leaf(PaneType::MarkDown {
                                    name,
                                    content: buffer,
                                });
                            }
                            Some("qql") => {
                                self.dock_aluminum
                                    .backend_tx
                                    .send(Event::EngineEvent(
                                        events::events::engine::EngineEvent::Start {
                                            filename: name.clone(),
                                        },
                                    ))
                                    .unwrap_or_else(|e| {
                                        log::error!("Failed to send NewCodeExecution event: {}", e);
                                    });
                                self.dock_state.push_to_focused_leaf(PaneType::CodeEditor {
                                    file_name: name.clone(),
                                    buffer,
                                });
                            }
                            _ => {
                                self.dock_state.push_to_focused_leaf(PaneType::CodeEditor {
                                    file_name: name.clone(),
                                    buffer,
                                });
                            }
                        }

                        updated = true;
                    }
                    events::events::dock::DockEvent::ShowGraph { name } => {
                        log::info!("Showing graph for: {}", name);
                        let (tx, rx) = crossbeam_channel::unbounded::<Output>();
                        self.panel_channels
                            .entry(name.clone())
                            .or_insert_with(Vec::new)
                            .push((tx, rx.clone()));
                        self.dock_state.push_to_focused_leaf(PaneType::GraphView {
                            title: name,
                            data: rx,
                            draw_graph: None,
                        });
                        updated = true;
                    }
                    events::events::dock::DockEvent::ShowTrades { name } => {
                        log::info!("Showing trades for: {}", name);
                        let (tx, rx) = crossbeam_channel::unbounded::<Output>();
                        self.panel_channels
                            .entry(name.clone())
                            .or_insert_with(Vec::new)
                            .push((tx, rx.clone()));
                        self.dock_state.push_to_focused_leaf(PaneType::TradeView {
                            title: name,
                            summary: rx,
                            trade_summary: None,
                        });
                        updated = true;
                    }
                    events::events::dock::DockEvent::UpdateOutput { name, content } => {
                        // Implement output updating logic here
                        if let Some(channels) = self.panel_channels.get(&name) {
                            for ch in channels {
                                ch.0.send(content.clone()).unwrap_or_else(|e| {
                                    log::error!("Failed to send output to channel: {}", e);
                                });
                            }
                        } else {
                            log::warn!("No channels found for panel: {}", name);
                        }
                    }
                    _ => {
                        log::warn!("Unsupported DockEvent received in UI");
                    }
                }
            } else {
                log::warn!("Unsupported event type received in Dock");
            }
        }
        if updated {
            ui.ctx().request_repaint();
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, tab_viewer: &mut MyTabViewer) {
        // Inherit the look and feel from egui.
        self.pump_snapshots(ui);
        let mut style = Style::from_egui(ui.style());

        // Modify a few fields.
        style.main_surface_border_rounding = CornerRadius::from(0.0);
        style.main_surface_border_stroke = egui::Stroke::new(0.0, egui::Color32::TRANSPARENT);

        style.overlay.overlay_type = OverlayType::Widgets;
        style.buttons.add_tab_align = TabAddAlign::Left;
        style.main_surface_border_rounding = CornerRadius::from(0.0);

        style.tab.active.corner_radius = CornerRadius::from(0.0);
        style.tab.active.bg_fill = theme::get_mode_theme(ui.ctx()).base;
        style.tab.inactive.corner_radius = CornerRadius::from(0.0);
        style.tab.inactive.bg_fill = theme::get_mode_theme(ui.ctx()).base;
        style.tab.focused.corner_radius = CornerRadius::from(0.0);
        style.tab.focused.bg_fill = theme::get_mode_theme(ui.ctx()).base;
        style.tab.focused.outline_color = theme::get_mode_theme(ui.ctx()).blue;
        style.tab.hovered.corner_radius = CornerRadius::from(0.0);
        style.tab.hovered.bg_fill = theme::get_mode_theme(ui.ctx()).base;

        style.tab.tab_body.corner_radius = CornerRadius::from(0.0);
        style.tab.tab_body.bg_fill = theme::get_mode_theme(ui.ctx()).base;
        style.tab.tab_body.stroke = egui::Stroke::new(0.0, egui::Color32::TRANSPARENT);

        style.tab_bar.corner_radius = CornerRadius::from(0.0);
        style.tab_bar.bg_fill = theme::get_mode_theme(ui.ctx()).crust;
        style.tab.tab_body.inner_margin = Margin {
            left: 6,
            right: 6,
            top: 4,
            bottom: 4,
        };

        style.separator = SeparatorStyle {
            width: 1.0,
            extra_interact_width: 2.0,
            extra: 175.0,
            color_idle: theme::get_mode_theme(ui.ctx()).crust,
            color_hovered: theme::get_mode_theme(ui.ctx()).crust,
            color_dragged: theme::get_mode_theme(ui.ctx()).crust,
        };

        DockArea::new(&mut self.dock_state)
            .show_add_buttons(false)
            .show_close_buttons(true)
            .show_leaf_close_all_buttons(false)
            .show_leaf_collapse_buttons(false)
            .style(style)
            .show_inside(ui, tab_viewer);
    }
}
