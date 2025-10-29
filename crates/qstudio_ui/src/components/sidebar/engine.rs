use busbar::Aluminum;
use egui::RichText;
use qstudio_tcp::Client;
use std::sync::Arc;

use events::events::engine::EngineEvent;
use events::Event;

#[derive(Clone, Debug)]
struct EngineItem {
    name: String,
    status: String,
    msg: Option<String>,
    ui_aluminum: Arc<Aluminum<(Client, events::Event)>>,
    only_client: Client,
}

impl EngineItem {
    fn new(
        name: String,
        status: String,
        msg: Option<String>,
        ui_aluminum: Arc<Aluminum<(Client, events::Event)>>,
        only_client: Client,
    ) -> Self {
        Self {
            name,
            status,
            msg,
            ui_aluminum,
            only_client,
        }
    }

    fn button_ui(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match self.status.as_str() {
                "running" => {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(egui_material_icons::icons::ICON_STOP)
                                    .color(egui::Color32::ORANGE)
                                    .size(16.0),
                            )
                            .fill(egui::Color32::TRANSPARENT),
                        )
                        .on_hover_text("Stop engine")
                        .clicked()
                    {
                        // Handle stop button click
                    }
                }
                _ => {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(egui_material_icons::icons::ICON_START)
                                    .color(egui::Color32::from_rgb(76, 175, 80)) // Calmer green (Material Green 500)
                                    .size(16.0),
                            )
                            .fill(egui::Color32::TRANSPARENT),
                        )
                        .on_hover_text("Start engine")
                        .clicked()
                    {
                        // Handle start button click
                        self.ui_aluminum
                            .frontend_tx
                            .send((
                                self.only_client.clone(),
                                Event::EngineEvent(EngineEvent::Start {
                                    filename: self.name.clone(),
                                }),
                            ))
                            .unwrap_or_else(|e| {
                                log::error!("Failed to send StartEngine event: {}", e);
                            });
                    }
                }
            }

            if ui
                .add(
                    egui::Button::new(
                        RichText::new(egui_material_icons::icons::ICON_RESTORE)
                            .color(egui::Color32::from_rgb(100, 149, 237)) // Calmer blue (Cornflower Blue)
                            .size(16.0),
                    )
                    .fill(egui::Color32::TRANSPARENT),
                )
                .on_hover_text("Restore engine")
                .clicked()
            {
                // Handle restore button click
            }

            if ui
                .add(
                    egui::Button::new(
                        RichText::new(egui_material_icons::icons::ICON_DELETE)
                            .color(egui::Color32::RED)
                            .size(16.0),
                    )
                    .fill(egui::Color32::TRANSPARENT),
                )
                .on_hover_text("Delete engine")
                .clicked()
            {
                // Handle delete button click
            }

            if ui
                .add(
                    egui::Button::new(
                        RichText::new(egui_material_icons::icons::ICON_WORKFLOW).size(16.0),
                    )
                    .fill(egui::Color32::TRANSPARENT),
                )
                .on_hover_text("Code Flow Chart")
                .clicked()
            {
                // Handle view button click
                self.ui_aluminum
                    .frontend_tx
                    .send((
                        self.only_client.clone(),
                        Event::UiEvent(events::UiEvent::ShowTables {
                            name: self.name.clone(),
                        }),
                    ))
                    .unwrap_or_else(|e| {
                        log::error!("Failed to send ShowTrades event: {}", e);
                    });
            }

            if ui
                .add(
                    egui::Button::new(
                        RichText::new(egui_material_icons::icons::ICON_CHART_DATA).size(16.0),
                    )
                    .fill(egui::Color32::TRANSPARENT),
                )
                .on_hover_text("View stock chart")
                .clicked()
            {
                // Handle view button click
                self.ui_aluminum
                    .frontend_tx
                    .send((
                        self.only_client.clone(),
                        Event::UiEvent(events::UiEvent::ShowGraph {
                            name: self.name.clone(),
                        }),
                    ))
                    .unwrap_or_else(|e| {
                        log::error!("Failed to send ShowGraph event: {}", e);
                    });
            }
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(egui_material_icons::icons::ICON_TABLE_VIEW).size(16.0),
                    )
                    .fill(egui::Color32::TRANSPARENT),
                )
                .on_hover_text("View stock table")
                .clicked()
            {
                // Handle view button click
            }
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(egui_material_icons::icons::ICON_CURRENCY_EXCHANGE)
                            .size(16.0),
                    )
                    .fill(egui::Color32::TRANSPARENT),
                )
                .on_hover_text("View trade summary")
                .clicked()
            {
                // Handle view button click
                self.ui_aluminum
                    .frontend_tx
                    .send((
                        self.only_client.clone(),
                        Event::UiEvent(events::UiEvent::ShowTrades {
                            name: self.name.clone(),
                        }),
                    ))
                    .unwrap_or_else(|e| {
                        log::error!("Failed to send ShowTrades event: {}", e);
                    });
            }
        });
    }

    fn ui(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(&self.name)
            .default_open(true)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(format!("Status: {}", self.status));
                    if let Some(msg) = &self.msg {
                        ui.label(format!("Message: {}", msg));
                    }
                    self.button_ui(ui);
                });
            });
    }
}

#[derive(Clone, Debug)]
pub struct ActiveEngines {
    engines: Vec<EngineItem>,
    engine_aluminum: Arc<Aluminum<(Client, events::Event)>>,
    only_client: Client,
}

impl ActiveEngines {
    pub fn new(
        engine_aluminum: Arc<Aluminum<(Client, events::Event)>>,
        only_client: Client,
    ) -> Self {
        ActiveEngines {
            engines: Vec::new(),
            engine_aluminum,
            only_client,
        }
    }

    fn pump_snapshots(&mut self, ctx: &egui::Context) {
        let mut update = false;

        while let Ok(ev) = self.engine_aluminum.engine_rx.try_recv() {
            log::info!("Engine sidebar received event: {}", ev.1);
            if let Event::EngineEvent(engine_event) = ev.1 {
                match engine_event {
                    events::events::engine::EngineEvent::NewEngineMonitor { name, status } => {
                        // Update or add the engine item
                        if let Some(engine) = self.engines.iter_mut().find(|e| e.name == name) {
                            engine.status = status;
                            engine.msg = None;
                        } else {
                            self.engines.push(EngineItem::new(
                                name,
                                status,
                                None,
                                Arc::clone(&self.engine_aluminum),
                                self.only_client.clone(),
                            ));
                        }
                        update = true;
                    }
                    _ => {
                        log::warn!("Unsupported EngineEvent received in UI");
                    }
                }
            }
        }

        if update {
            ctx.request_repaint();
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.pump_snapshots(ui.ctx());
        ui.vertical(|ui| {
            ui.set_min_height(ui.available_height());
            ui.add_space(12.0);
            ui.heading("Active Engines");
            ui.separator();
            if self.engines.is_empty() {
                ui.label("No active engines.");
            } else {
                for engine in &mut self.engines {
                    engine.ui(ui);
                }
            }
        });
    }
}
